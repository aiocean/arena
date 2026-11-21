"""The trainer's policy network and the value head it alone uses (a critic trained by HL-Gauss classification over
fixed bins). A bot is the policy exported as ONNX (`to_onnx`): the observation, float32 [1, OBS], to every act's
logit, [1, ACTIONS]. Nothing in the arena fixes its size: `width`, `hidden` and `blocks` are the trainer's to choose,
and `from_onnx` reads them back from a model.
"""

import io
import threading
import math

import torch
from torch import nn

WIDTH, HIDDEN, GROUP, BLOCKS = 128, 256, 8, 2

# The observation's repeated things (`crates/arena-mind/src/policy/observe.rs`, ABI 5's 2055 inputs), each read by one
# encoder shared over its copies (`Policy(entities=...)`): (name, copies, the ranges one copy reads as (start, length,
# stride)). A slot of mine is its card, its build and why it can or cannot be cast; a slot of the foe's, its card and
# its band.
ENTITIES = [
    ("slot", 6, [(102, 33, 33), (327, 148, 148), (1741, 9, 9)]),
    ("threat", 4, [(54, 12, 12)]),
    ("unit", 8, [(1441, 12, 12)]),
    ("foe_slot", 6, [(1537, 34, 34), (1795, 4, 4)]),
    ("in_play", 10, [(1839, 5, 5)]),
    ("my_act", 3, [(1889, 30, 30)]),
    ("foe_act", 4, [(1979, 19, 19)]),
]
ENTITY_OBS = 2055
# ABI 6 appends my older acts, the foe's older ones and, for each kind of skill, when the foe last started one
ENTITIES_ABI6 = ENTITIES + [
    ("my_old_act", 5, [(2055, 30, 30)]),
    ("foe_old_act", 4, [(2205, 19, 19)]),
    ("foe_kind", 9, [(2281, 2, 2)]),
]
ENTITY_KINDS = {2055: ENTITIES, 2299: ENTITIES_ABI6}


def entities_of(obs):
    """The repeated things of an observation of `obs` inputs (ABI 5's or ABI 6's)."""
    if obs not in ENTITY_KINDS:
        raise ValueError(f"the entity encoders read ABI 5's or 6's inputs ({sorted(ENTITY_KINDS)}), not {obs}")
    return ENTITY_KINDS[obs]


def entity_index(copies, ranges):
    """The observation's indices each copy of an entity reads, [copies, its width]."""
    return torch.tensor([[start + i * stride + j for start, n, stride in ranges for j in range(n)] for i in range(copies)])


class Entities(nn.Module):
    """One small MLP per kind of repeated thing, shared over its copies; the copies' codes kept in order (a slot's
    place is the act that casts it, a threat's is how near it is) and projected to the trunk's width."""

    def __init__(self, obs, width, dim, heads=0):
        super().__init__()
        kinds = entities_of(obs)
        self.names = [name for name, _, _ in kinds]
        for name, copies, ranges in kinds:
            idx = entity_index(copies, ranges)
            self.register_buffer(f"idx_{name}", idx, persistent=False)
            setattr(self, name, nn.Sequential(nn.Linear(idx.shape[1], dim), nn.ReLU(), nn.Linear(dim, dim), nn.ReLU()))
        tokens = sum(c for _, c, _ in kinds)
        self.tokens = tokens
        self.proj = nn.Linear(tokens * dim, width)
        # with `heads`, one layer of self-attention over every copy of every kind (41 tokens), so a code also reads
        # the others (which of my slots answers which threat); each token knows its kind and place by a learnt vector.
        # The projections are 2-D Linears over the tokens flattened into the batch, so ONNX keeps their names
        self.heads = heads
        if heads:
            if dim % heads:
                raise ValueError(f"an entity width of {dim} does not split into {heads} heads")
            self.place = nn.Parameter(torch.zeros(tokens, dim))
            self.att_ln = nn.LayerNorm(dim, eps=1e-5)
            self.att_qkv = nn.Linear(dim, 3 * dim)
            self.att_out = nn.Linear(dim, dim)

    def attend(self, x):
        """x [batch, tokens, dim] -> x + attention(ln(x))"""
        b, t, d = x.shape
        h = self.heads
        qkv = self.att_qkv(self.att_ln(x).reshape(-1, d)).reshape(b, t, 3, h, d // h).permute(2, 0, 3, 1, 4)
        q, k, v = qkv[0], qkv[1], qkv[2]
        w = torch.softmax(q @ k.transpose(-1, -2) * (d // h) ** -0.5, -1)
        y = (w @ v).transpose(1, 2).reshape(-1, d)
        return x + self.att_out(y).reshape(b, t, d)

    def forward(self, obs):
        codes = []
        for name in self.names:
            idx = getattr(self, f"idx_{name}")
            copies, one = idx.shape
            # the copies flattened into the batch: a 2-D Linear exports as a Gemm whose weight keeps its name
            # (`from_onnx` finds it), where a 3-D one becomes an anonymous MatMul
            x = obs[:, idx].reshape(-1, one)
            codes.append(getattr(self, name)(x).reshape(obs.shape[0], copies, -1))
        x = torch.cat(codes, 1)
        if self.heads:
            x = self.attend(x + self.place)
        return self.proj(x.reshape(obs.shape[0], -1))


class Recurrent(nn.Module):
    """A GRU cell over the trunk's output, its state carried from decision to decision (cleared each round), and
    what it adds back to the trunk's output. The add starts at zero, so a policy given one plays as it did; training
    then learns what to remember. The gates are 2-D Linears, which ONNX keeps by name."""

    def __init__(self, width, size):
        super().__init__()
        self.size = size
        self.x = nn.Linear(width, 3 * size)
        self.h = nn.Linear(size, 3 * size)
        self.out = nn.Linear(size, width)
        nn.init.zeros_(self.out.weight)
        nn.init.zeros_(self.out.bias)

    def forward(self, z, h):
        """(what to add to z, the next state)"""
        xr, xu, xn = self.x(z).chunk(3, -1)
        hr, hu, hn = self.h(h).chunk(3, -1)
        r = torch.sigmoid(xr + hr)
        u = torch.sigmoid(xu + hu)
        n = torch.tanh(xn + r * hn)
        h = (1 - u) * n + u * h
        return self.out(h), h


class Block(nn.Module):
    """x + down(relu(up(ln(x))))"""

    def __init__(self, width, hidden):
        super().__init__()
        self.ln = nn.LayerNorm(width, eps=1e-5)
        self.up = nn.Linear(width, hidden)
        self.down = nn.Linear(hidden, width)

    def forward(self, x):
        return x + self.down(torch.relu(self.up(self.ln(x))))


class Policy(nn.Module):
    """relu(in), `blocks` residual blocks, a LayerNorm, a simplicial embedding (softmaxes of 8), the policy head; and
    the value head (bins over [-1.5, 1.5]) from the trunk's normalised output."""

    def __init__(
        self, obs, actions, width=WIDTH, hidden=HIDDEN, blocks=BLOCKS, bins=101, vmax=1.5, privileged=0, entities=0,
        attention=0,
        recurrent=0,
    ):
        super().__init__()
        # the bigger brain (phase D): with `entities` (an encoder's width), each repeated thing of the observation is
        # also read by an encoder shared over its copies (`Entities`), added to `inp`'s reading of the whole
        # with `attention` (heads), the encoders' codes also attend to one another (`Entities.attend`)
        if attention and not entities:
            raise ValueError("attention runs over the entity encoders' codes: it needs entities")
        self.ent = Entities(obs, width, entities, attention) if entities else None
        # the asymmetric critic (`ppo.py --privileged`): what only the critic sees (`VecEnv.privileged`), embedded and
        # added to the trunk's output before the value head; the policy head never reads it, so the bot is unchanged
        self.priv = (
            nn.Sequential(nn.Linear(privileged, width), nn.ReLU(), nn.Linear(width, width), nn.LayerNorm(width, eps=1e-5))
            if privileged
            else None
        )
        # with `recurrent` (a state's size), a GRU carries what it saw from decision to decision (`Recurrent`): the
        # bot keeps the state between calls and clears it each round, `ppo.py` trains it over each env's steps in order
        self.rec = Recurrent(width, recurrent) if recurrent else None
        self.inp = nn.Linear(obs, width)
        self.blocks = nn.ModuleList([Block(width, hidden) for _ in range(blocks)])
        self.out_ln = nn.LayerNorm(width, eps=1e-5)
        self.sem = nn.Linear(width, width)
        self.pi = nn.Linear(width, actions)
        self.value = nn.Sequential(nn.Linear(width, width), nn.ReLU(), nn.Linear(width, bins))
        self.bins = bins
        self.register_buffer("support", torch.linspace(-vmax, vmax, bins + 1))
        self.sigma = 0.75 * (2 * vmax / bins)
        self.width = width

    def trunk(self, obs):
        """The trunk's normalised output, which both heads read."""
        h = self.inp(obs)
        if self.ent is not None:
            h = h + self.ent(obs)
        h = torch.relu(h)
        for b in self.blocks:
            h = b(h)
        return self.out_ln(h)

    def logits(self, z):
        """The policy head: every act's logit, legal or not, from the trunk's output."""
        s = self.sem(z).view(*z.shape[:-1], self.width // GROUP, GROUP).softmax(-1).flatten(-2)
        return self.pi(s)

    def forward(self, obs, mask, priv=None):
        """Masked logits (an illegal act gets -1e8) and the value logits (with `priv`, the critic's extra inputs, when
        the policy has an asymmetric critic). A recurrent policy is read from a fresh state (`step` carries one)."""
        if self.rec is not None:
            logits, value, _ = self.step(obs, mask, self.fresh(obs.shape[0], obs.device), priv)
            return logits, value
        z = self.trunk(obs)
        v = z + self.priv(priv) if self.priv is not None and priv is not None else z
        return self.logits(z).masked_fill(~mask, -1e8), self.value(v)

    def fresh(self, n, device=None):
        """A recurrent policy's state at a round's start."""
        return torch.zeros(n, self.rec.size, device=device)

    def step(self, obs, mask, h, priv=None):
        """A recurrent policy's decision: masked logits, the value logits and the next state."""
        z = self.trunk(obs)
        add, h = self.rec(z, h)
        z = z + add
        v = z + self.priv(priv) if self.priv is not None and priv is not None else z
        return self.logits(z).masked_fill(~mask, -1e8), self.value(v), h

    def value_of(self, value_logits):
        """The critic's expected return."""
        centres = (self.support[:-1] + self.support[1:]) / 2
        return (value_logits.softmax(-1) * centres).sum(-1)


def size_of(policy):
    """A policy's size, as a checkpoint's "size" keeps it and `Policy(**size)` takes it."""
    size = dict(width=policy.width, hidden=policy.blocks[0].up.out_features, blocks=len(policy.blocks))
    if policy.ent is not None:
        size["entities"] = policy.ent.proj.in_features // policy.ent.tokens
        if policy.ent.heads:
            size["attention"] = policy.ent.heads
    if policy.rec is not None:
        size["recurrent"] = policy.rec.size
    return size


def without_priv(model):
    """A state dict without the asymmetric critic's layers (`priv.*`): what the policy and its exports need."""
    return {k: v for k, v in model.items() if not k.startswith("priv.")}


def hl_gauss(returns, support, sigma):
    """HL-Gauss targets: a Gaussian of width sigma round each return, integrated over each bin."""
    cdf = torch.special.erf((support[None, :] - returns[:, None].clamp(support[0], support[-1])) / (math.sqrt(2) * sigma))
    p = cdf[:, 1:] - cdf[:, :-1]
    return p / p.sum(-1, keepdim=True).clamp_min(1e-12)


class Logits(nn.Module):
    """The policy as a bot runs it: the observation, float32 [1, OBS], to every act's logit, [1, ACTIONS]."""

    def __init__(self, policy):
        super().__init__()
        self.policy = policy

    def forward(self, obs):
        return self.policy.logits(self.policy.trunk(obs))


class RecurrentLogits(nn.Module):
    """A recurrent policy as a bot runs it: (obs [1, OBS], h [1, size]) to (logits [1, ACTIONS], the next h)."""

    def __init__(self, policy):
        super().__init__()
        self.policy = policy

    def forward(self, obs, h):
        z = self.policy.trunk(obs)
        add, h = self.policy.rec(z, h)
        return self.policy.logits(z + add), h


# torch's ONNX exporter keeps global state: one export at a time (the pool fights its games on threads, and each may
# carry an older model over through an export)
_EXPORT = threading.Lock()


def to_onnx(policy):
    """The policy as an ONNX model's bytes (opset 17, the legacy exporter: tract reads it), what `bots/onnx` builds
    into a bot module and the trainer's envs seat (`VecEnv.set_model`)."""
    was = policy.training
    policy.eval()
    buf = io.BytesIO()
    obs = policy.inp.in_features
    rec = policy.rec is not None
    with _EXPORT, torch.no_grad():
        torch.onnx.export(
            (RecurrentLogits if rec else Logits)(policy).cpu(),
            (torch.zeros(1, obs), torch.zeros(1, policy.rec.size)) if rec else (torch.zeros(1, obs),),
            buf,
            opset_version=17,
            dynamo=False,
            input_names=["obs", "h"] if rec else ["obs"],
            output_names=["logits", "h_out"] if rec else ["logits"],
        )
    policy.train(was)
    data = buf.getvalue()
    if policy.ent is not None and policy.ent.heads:
        # what the weights' shapes cannot tell `from_onnx`: the attention's heads
        import onnx

        model = onnx.load_from_string(data)
        onnx.helper.set_model_props(model, {"attention_heads": str(policy.ent.heads)})
        data = model.SerializeToString()
    return data


def _initializers(data):
    """An ONNX model's weights by the policy's own parameter names. The exporter stores tensors of equal values once
    (a fresh model's LayerNorms) and names the others through Identity nodes, which this follows."""
    import onnx
    from onnx import numpy_helper

    model = onnx.load_from_string(data)
    tensors = {i.name: torch.from_numpy(numpy_helper.to_array(i).copy()) for i in model.graph.initializer}
    for node in model.graph.node:
        if node.op_type == "Identity" and node.input[0] in tensors:
            tensors[node.output[0]] = tensors[node.input[0]]
    return {name.removeprefix("policy."): t for name, t in tensors.items()}


def from_onnx(data, obs, actions):
    """A policy of the size an ONNX model `to_onnx` wrote has, with its weights (the value head fresh)."""
    found = _initializers(data)
    width, hidden = found["blocks.0.up.weight"].shape[1], found["blocks.0.up.weight"].shape[0]
    blocks = len([k for k in found if k.startswith("blocks.") and k.endswith(".up.weight")])
    entities = found["ent.slot.0.weight"].shape[0] if "ent.slot.0.weight" in found else 0
    attention = 0
    if "ent.att_qkv.weight" in found:
        import onnx

        props = {p.key: p.value for p in onnx.load_from_string(data).metadata_props}
        attention = int(props["attention_heads"])
    recurrent = found["rec.h.weight"].shape[1] if "rec.h.weight" in found else 0
    policy = Policy(
        obs, actions, width=width, hidden=hidden, blocks=blocks, entities=entities, attention=attention, recurrent=recurrent
    )
    load_onnx(policy, data)
    return policy


def load_onnx(policy, data):
    """Load an ONNX model's weights (as `to_onnx` writes them) into `policy`; its value head is left as it is."""
    found = _initializers(data)
    own = dict(policy.named_parameters())
    with torch.no_grad():
        for name, t in own.items():
            if name.startswith("value."):
                continue
            if name not in found:
                raise ValueError(f"no tensor {name} in the model")
            if tuple(found[name].shape) != tuple(t.shape):
                raise ValueError(f"tensor {name} is {tuple(found[name].shape)}, want {tuple(t.shape)}")
            t.copy_(found[name])
