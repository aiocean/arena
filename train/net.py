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

    def __init__(self, obs, actions, width=WIDTH, hidden=HIDDEN, blocks=BLOCKS, bins=101, vmax=1.5, privileged=0):
        super().__init__()
        # the asymmetric critic (`ppo.py --privileged`): what only the critic sees (`VecEnv.privileged`), embedded and
        # added to the trunk's output before the value head; the policy head never reads it, so the bot is unchanged
        self.priv = (
            nn.Sequential(nn.Linear(privileged, width), nn.ReLU(), nn.Linear(width, width), nn.LayerNorm(width, eps=1e-5))
            if privileged
            else None
        )
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
        h = torch.relu(self.inp(obs))
        for b in self.blocks:
            h = b(h)
        return self.out_ln(h)

    def logits(self, z):
        """The policy head: every act's logit, legal or not, from the trunk's output."""
        s = self.sem(z).view(*z.shape[:-1], self.width // GROUP, GROUP).softmax(-1).flatten(-2)
        return self.pi(s)

    def forward(self, obs, mask, priv=None):
        """Masked logits (an illegal act gets -1e8) and the value logits (with `priv`, the critic's extra inputs, when
        the policy has an asymmetric critic)."""
        z = self.trunk(obs)
        v = z + self.priv(priv) if self.priv is not None and priv is not None else z
        return self.logits(z).masked_fill(~mask, -1e8), self.value(v)

    def value_of(self, value_logits):
        """The critic's expected return."""
        centres = (self.support[:-1] + self.support[1:]) / 2
        return (value_logits.softmax(-1) * centres).sum(-1)


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
    with _EXPORT, torch.no_grad():
        torch.onnx.export(
            Logits(policy).cpu(),
            (torch.zeros(1, obs),),
            buf,
            opset_version=17,
            dynamo=False,
            input_names=["obs"],
            output_names=["logits"],
        )
    policy.train(was)
    return buf.getvalue()


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
    policy = Policy(obs, actions, width=width, hidden=hidden, blocks=blocks)
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
