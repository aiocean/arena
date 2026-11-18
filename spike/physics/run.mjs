const m = await import(process.argv[2] + '/physics_spike.js');
const [seeds, secs] = [Number(process.argv[3] ?? 20), Number(process.argv[4] ?? 120)];
const t = performance.now();
process.stdout.write(m.default.wasm_report(seeds, secs));
console.error(`${((performance.now() - t) / 1000).toFixed(1)} s`);
