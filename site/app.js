const computeWorkers = Array.from({length:2}, () => new Worker(new URL("./compute-worker.js", import.meta.url), { type: "module" }));
let workerTask = 0;
const pending = new Map();
for (const worker of computeWorkers) worker.addEventListener("message", ({data}) => {
  const task = pending.get(data.id); if (!task) return; pending.delete(data.id);
  data.error ? task.reject(new Error(data.error)) : task.resolve(data.output);
});
function calculateOne(worker,input) {
  return new Promise((resolve,reject) => { const id=++workerTask; pending.set(id,{resolve,reject}); worker.postMessage({id,input}); });
}
async function calculateAsync(input) {
  if (input.only) return calculateOne(computeWorkers[workerTask++ % computeWorkers.length],input);
  const [tracking,fixed] = await Promise.all([
    calculateOne(computeWorkers[0],{...input,only:"tracking"}),
    calculateOne(computeWorkers[1],{...input,only:"fixed"})
  ]);
  const comparison=[...tracking.comparison,...fixed.comparison].sort((a,b)=>a.lcoe-b.lcoe);
  return {selected:comparison[0],comparison};
}

const $ = id => document.getElementById(id);
const weatherCache = new Map();
const PRESETS = {
  "Low Cost": {debt_cost:8,leverage:100,debt_term:20,equity_cost:0,itc:0,tax_rate:0,pv_cost:.530,bess_cost:62,integration:100,solar_om:4,bess_om:.8,bos_om:2,soft_om:0,om_escalator:0,fuel_escalator:0,soft_pct:0,depreciation:0},
  "Middle Cost": {debt_cost:9,leverage:100,debt_term:20,equity_cost:0,itc:0,tax_rate:0,pv_cost:.667,bess_cost:140,integration:200,solar_om:7,bess_om:1.7,bos_om:4,soft_om:0,om_escalator:0,fuel_escalator:0,soft_pct:0,depreciation:0},
  "High Cost": {debt_cost:10,leverage:100,debt_term:20,equity_cost:0,itc:0,tax_rate:0,pv_cost:.785,bess_cost:378,integration:300,solar_om:11,bess_om:2.5,bos_om:6,soft_om:0,om_escalator:0,fuel_escalator:0,soft_pct:0,depreciation:0},
  "High Cost, Cheaper Storage": {debt_cost:10,leverage:100,debt_term:20,equity_cost:0,itc:0,tax_rate:0,pv_cost:.785,bess_cost:.175,integration:300,solar_om:11,bess_om:2.5,bos_om:6,soft_om:0,om_escalator:0,fuel_escalator:0,soft_pct:0,depreciation:0}
};
const DEFAULTS = {debt_cost:8,leverage:100,debt_term:20,equity_cost:0,itc:0,tax_rate:0,pv_cost:.600,bess_cost:200,gen_capex:635,integration:300,soft_pct:0,fuel_price:8,fuel_escalator:0,solar_om:11,bess_om:2.5,gen_om:15,gen_var:.005,bos_om:6,soft_om:0,om_escalator:0,depreciation:0};

function setInputs(values) {
  for (const [key,value] of Object.entries(values)) if ($(key.replaceAll("_","-"))) $(key.replaceAll("_","-")).value=value;
}
function v(id) { return Number($(id).value); }
function fixedOrientation(lat) {
  const latBin=5*Math.floor(Math.abs(lat)/5+.5);
  return 5*Math.floor(.9*latBin/5+.5);
}
async function getProfiles(lat,lon,tilt) {
  const cacheKey=`${lat.toFixed(5)},${lon.toFixed(5)}`;
  if(weatherCache.has(cacheKey)) return weatherCache.get(cacheKey);
  const base=$("proxy").value.trim().replace(/\/$/,"");
  const api=`https://re.jrc.ec.europa.eu/api/tmy?lat=${encodeURIComponent(lat)}&lon=${encodeURIComponent(lon)}&outputformat=json`;
  const response=await fetch(`${base}/${api}`,{cache:"default"});
  if(!response.ok) throw new Error(`PVGIS request failed (${response.status}). ${await response.text()}`);
  const data=await response.json();
  const rows=data?.outputs?.tmy_hourly;
  if(!rows?.length) throw new Error("PVGIS returned no hourly TMY data for these coordinates.");
  const profiles={weather:rows};
  profiles.source=data?.inputs?.meteo_data?.radiation_db||"PVGIS default";
  weatherCache.set(cacheKey,profiles);
  return profiles;
}

function readParams() {
  const genType=$("gen-type").value;
  const vals={
    mode:"run",target:95,standalone:false,only:$("tracking-only").checked?"tracking":$("fixed-only").checked?"fixed":"",
    load:v("load"),solar:v("solar"),bess:v("bess"),generator:v("generator"),gen_type:genType,
    debt_cost:v("debt-cost"),leverage:v("leverage"),debt_term:v("debt-term"),equity_cost:v("equity-cost"),itc:v("itc"),tax_rate:v("tax-rate"),
    pv_cost:v("pv-cost"),bess_cost:v("bess-cost"),gen_capex:v("gen-capex"),integration:v("integration"),soft_pct:v("soft-pct"),
    fuel_price:v("fuel-price"),fuel_escalator:v("fuel-escalator"),solar_om:v("solar-om"),bess_om:v("bess-om"),gen_om:v("gen-om"),gen_var:v("gen-var"),bos_om:v("bos-om"),soft_om:v("soft-om"),om_escalator:v("om-escalator"),
    depreciation:Array(20).fill(v("depreciation")),construction_years:2
  };
  return vals;
}

function fmt(x,n=2){return Number(x).toLocaleString(undefined,{minimumFractionDigits:n,maximumFractionDigits:n});}
function showResults(out,source,tilt){
  const r=out.selected; $("results").classList.remove("hidden");
  const rows=out.comparison.map(x=>`<tr><td>${x.configuration}${x.tilt!==null?` (${fmt(tilt,0)}°)`:""}</td><td>${fmt(x.pv_yield_mwh_per_mw,1)}</td><td>${fmt(x.solar_mw,0)} MW</td><td>${fmt(x.bess_mw,0)} MW</td><td>$${fmt(x.lcoe)}/MWh</td></tr>`).join("");
  $("result-content").innerHTML=`<p class="choice">Least-cost configuration: ${r.configuration}${r.tilt!==null?` · ${fmt(tilt,0)}° tilt, ${$("lat").value>=0?"south-facing":"north-facing"}`:""}</p><div class="result-grid"><div class="metric"><span>Calculated LCOE</span><strong>$${fmt(r.lcoe)}/MWh</strong></div><div class="metric"><span>Renewable share of served energy</span><strong>${fmt(r.renewable_pct)}%</strong></div><div class="metric"><span>Average annual gas generation</span><strong>${fmt(r.average_generator_mwh,0)} MWh</strong></div><div class="metric"><span>Annual load served</span><strong>${fmt(r.load_served_pct)}%</strong></div><div class="metric"><span>PVGIS solar database</span><strong>${source}</strong></div></div><table class="comparison"><thead><tr><th>Configuration</th><th>Annual PV yield (MWh/MW-DC)</th><th>Solar PV</th><th>BESS</th><th>LCOE</th></tr></thead><tbody>${rows}</tbody></table><p class="hint">${r.coarse_pairs?`Search evaluated ${r.coarse_pairs.toLocaleString()} coarse and ${r.refined_pairs.toLocaleString()} refined designs. `:""}${$("run").dataset.standalone==="true"?"Capacity search included a gas turbine equal to load, then removed generator capacity and all generator costs; unmet demand has no outage penalty.":""}Weather source: hourly PVGIS TMY. Fixed tilt uses the rounded 5° latitude-bin × 0.9 rule.</p>`;
}

async function execute(mode="run",target=95,standalone=false){
  const buttons=[...document.querySelectorAll("button")]; buttons.forEach(b=>b.disabled=true);
  const status=$("status"); status.className=""; status.textContent="Fetching PVGIS TMY and preparing tracking/fixed solar profiles…";
  try{
    const lat=v("lat"),lon=v("lon"); if(lat < -90||lat>90||lon < -180||lon>180) throw new Error("Enter latitude −90–90 and longitude −180–180.");
    const tilt=fixedOrientation(lat); $("tilt-note").textContent=`Fixed array comparison: ${tilt}° tilt facing ${lat>=0?"south":"north"}.`;
    const weatherStart=performance.now();
    const profiles=await getProfiles(lat,lon,tilt); status.textContent=`Weather loaded (${profiles.source}); running hourly dispatch and finance in WebAssembly…`;
    const params=readParams(); params.mode=mode; params.target=target; params.standalone=standalone; params.weather=profiles.weather; params.latitude=lat; params.longitude=lon; params.fixed_tilt=tilt;
    const computeStart=performance.now(); const out=await calculateAsync(params); const computeMs=performance.now()-computeStart;
    showResults(out,profiles.source,tilt); $("run").dataset.standalone=String(standalone);
    status.textContent=`Completed · WASM calculation ${computeMs.toFixed(0)} ms · weather request ${(computeStart-weatherStart).toFixed(0)} ms.`;
  }catch(e){status.className="error";status.textContent=e.message||String(e);}
  finally{buttons.forEach(b=>b.disabled=false);}
}

$("run").addEventListener("click",()=>execute());
document.querySelectorAll("[data-opt]").forEach(b=>b.addEventListener("click",()=>execute("optimize",Number(b.dataset.opt),b.dataset.standalone==="true")));
document.querySelectorAll("[data-preset]").forEach(b=>b.addEventListener("click",()=>{const name=b.dataset.preset;setInputs(PRESETS[name]);$("cost-preset-label").textContent=`Cost assumptions: ${name}`;}));
document.querySelectorAll("[data-gas]").forEach(b=>b.addEventListener("click",()=>{const high=b.dataset.gas==="higher";$("gen-type").value="Gas Turbine";$("gen-capex").value=high?1000:635;$("gen-om").value=high?32:15;$("fuel-price").value=high?11:8;$("gas-preset-label").textContent=`Gas assumptions: ${high?"Higher Cost Gas":"Lower Cost Gas (default)"}`;}));
$("tracking-only").addEventListener("change",()=>{if($("tracking-only").checked)$("fixed-only").checked=false;});
$("fixed-only").addEventListener("change",()=>{if($("fixed-only").checked)$("tracking-only").checked=false;});
