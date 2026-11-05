# Industrial Load Cost Optimizer (Rust/WASM)

This repository is a Rust/WebAssembly and GitHub Pages edition of the industrial-load solar, storage, and gas cost calculator. The original Python/Streamlit source is retained for reference; the published page runs dispatch, finance, and optimization locally in browser workers, comparing tracking and fixed arrays in parallel.

## Run locally

Install Rust, the `wasm32-unknown-unknown` target, and `wasm-pack`, then build and serve the static app:

```sh
wasm-pack build wasm --target web --release --out-dir ../site/pkg
python -m http.server 8000 --directory site
```

Open `http://localhost:8000`. GitHub Actions builds the WASM package and publishes `site/` to GitHub Pages whenever `main` changes.

## Solar resource and browser access

The app uses hourly PVGIS TMY data and the same pvlib model chain assumptions: NREL SPA solar position, single-axis tracking (north–south axis, backtracking, 2/7 ground coverage ratio), Hay-Davies sky diffuse, physical angle-of-incidence losses, SAPM cell temperature, and PVWatts DC/AC conversion. The fixed-array option uses the rounded latitude-bin tilt rule described in the app.

The PVGIS endpoint does not permit browser cross-origin requests, so the page's editable **PVGIS CORS relay URL** defaults to the public `proxy.cors.dev` relay. The relay receives the requested coordinates and may log them. For a controlled endpoint, `proxy/worker.js` is a small Cloudflare Worker that forwards only coordinate-validated PVGIS TMY GET requests; configure its `ALLOWED_ORIGINS` variable to the GitHub Pages origin and enter the Worker URL in the app.

## Calculation assumptions

The Rust engine preserves the source dispatch and finance rules: 20 operating years, 4-hour storage, 92% battery round-trip efficiency, 0.5% annual PV degradation, gas dispatch for any residual deficit, 2-year construction, and the original debt/tax/depreciation cash-flow equations. The source numerically applies a 0.35/20 annual battery-energy derate, which is a 35% decline by operating year 20 (its nearby comment labels this 0.35%, but that does not match the implemented number); the Rust version follows the number so results match. The default generator capacity is 500 MW. The four optimization actions search solar and battery capacity at a coarse load-scaled resolution, then refine promising designs in 100 MW increments; standalone cases size with load-equivalent gas and then remove generator capacity and costs before reporting the solar/storage LCOE.

The copy's historical Python UI remains available with `streamlit run app.py` for reference. GitHub Pages uses the Rust/WASM implementation in `wasm/` and the static interface in `site/`.
