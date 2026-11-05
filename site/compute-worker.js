import init, { calculate } from "./pkg/industrial_load_cost_optimizer_wasm.js";

await init();
self.addEventListener("message", ({data}) => {
  try { self.postMessage({id:data.id,output:calculate(data.input)}); }
  catch (error) { self.postMessage({id:data.id,error:error.message||String(error)}); }
});
