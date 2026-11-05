use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use solar_positioning::{Location, RefractionCorrection, SolarPositions};
use wasm_bindgen::prelude::*;

const YEARS: usize = 20;
const BATTERY_HOURS: f64 = 4.0;
const SQRT_RTE: f64 = 0.9591663046625439;
const BESS_DEG: f64 = 0.35 / 20.0;
const PV_DEG: f64 = 0.005;
const DC_AC: f64 = 1.2;

#[derive(Clone, Deserialize)]
struct Inputs {
    #[serde(default)]
    profile: Vec<f64>,
    #[serde(default)]
    tracking: Vec<f64>,
    #[serde(default)]
    fixed: Vec<f64>,
    #[serde(default)]
    weather: Vec<WeatherRow>,
    #[serde(default)]
    latitude: f64,
    #[serde(default)]
    longitude: f64,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    target: f64,
    #[serde(default)]
    standalone: bool,
    #[serde(default)]
    only: String,
    #[serde(default = "default_fixed_tilt")]
    fixed_tilt: f64,
    #[serde(default = "default_load")]
    load: f64,
    #[serde(default = "default_solar")]
    solar: i32,
    #[serde(default = "default_bess")]
    bess: i32,
    #[serde(default = "default_generator")]
    generator: f64,
    #[serde(default = "default_gen_type")]
    gen_type: String,
    #[serde(default = "default_debt")]
    debt_cost: f64,
    #[serde(default = "default_leverage")]
    leverage: f64,
    #[serde(default = "default_debt_term")]
    debt_term: usize,
    #[serde(default)]
    equity_cost: f64,
    #[serde(default)]
    itc: f64,
    #[serde(default)]
    tax_rate: f64,
    #[serde(default = "default_pv_cost")]
    pv_cost: f64,
    #[serde(default = "default_bess_cost")]
    bess_cost: f64,
    #[serde(default = "default_gen_capex")]
    gen_capex: f64,
    #[serde(default = "default_integration")]
    integration: f64,
    #[serde(default)]
    soft_pct: f64,
    #[serde(default = "default_fuel")]
    fuel_price: f64,
    #[serde(default)]
    fuel_escalator: f64,
    #[serde(default = "default_solar_om")]
    solar_om: f64,
    #[serde(default = "default_bess_om")]
    bess_om: f64,
    #[serde(default = "default_gen_om")]
    gen_om: f64,
    #[serde(default = "default_gen_var")]
    gen_var: f64,
    #[serde(default = "default_bos_om")]
    bos_om: f64,
    #[serde(default)]
    soft_om: f64,
    #[serde(default)]
    om_escalator: f64,
    #[serde(default = "default_construction")]
    construction_years: usize,
    #[serde(default = "zero_depreciation")]
    depreciation: Vec<f64>,
}

#[derive(Clone, Deserialize)]
struct WeatherRow {
    #[serde(rename = "time(UTC)")]
    time_utc: String,
    #[serde(rename = "G(h)")]
    ghi: f64,
    #[serde(rename = "Gb(n)")]
    dni: f64,
    #[serde(rename = "Gd(h)")]
    dhi: f64,
    #[serde(rename = "T2m")]
    temp_air: f64,
    #[serde(rename = "WS10m")]
    wind: f64,
}

fn default_load() -> f64 {
    500.0
}
fn default_solar() -> i32 {
    2500
}
fn default_bess() -> i32 {
    2500
}
fn default_generator() -> f64 {
    500.0
}
fn default_gen_type() -> String {
    "Gas Turbine".into()
}
fn default_debt() -> f64 {
    8.0
}
fn default_leverage() -> f64 {
    100.0
}
fn default_debt_term() -> usize {
    20
}
fn default_pv_cost() -> f64 {
    0.6
}
fn default_bess_cost() -> f64 {
    200.0
}
fn default_gen_capex() -> f64 {
    635.0
}
fn default_integration() -> f64 {
    300.0
}
fn default_fuel() -> f64 {
    8.0
}
fn default_solar_om() -> f64 {
    11.0
}
fn default_bess_om() -> f64 {
    2.5
}
fn default_gen_om() -> f64 {
    15.0
}
fn default_gen_var() -> f64 {
    0.005
}
fn default_bos_om() -> f64 {
    6.0
}
fn default_construction() -> usize {
    2
}
fn default_fixed_tilt() -> f64 {
    20.0
}
fn zero_depreciation() -> Vec<f64> {
    vec![0.0; YEARS]
}

fn fixed_orientation(latitude: f64) -> (f64, f64) {
    let lat_bin = 5.0 * (latitude.abs() / 5.0 + 0.5).floor();
    let tilt = (5.0 * (0.9 * lat_bin / 5.0 + 0.5).floor()).min(90.0);
    (tilt, if latitude >= 0.0 { 180.0 } else { 0.0 })
}

fn d2r(x: f64) -> f64 {
    x.to_radians()
}
fn r2d(x: f64) -> f64 {
    x.to_degrees()
}

fn plane_ac(
    row: &WeatherRow,
    zenith: f64,
    sun_azimuth: f64,
    day: u32,
    fixed: bool,
    tilt: f64,
    azimuth: f64,
) -> f64 {
    let (tilt, azimuth) = if fixed {
        (tilt, azimuth)
    } else {
        let z = d2r(zenith);
        let az = d2r(sun_azimuth);
        let mut theta = r2d((z.sin() * az.sin()).atan2(z.cos()));
        if zenith > 90.0 {
            return 0.0;
        }
        let shade = (3.5 * d2r(theta).cos()).abs();
        if shade < 1.0 {
            theta += -theta.signum() * r2d(shade.acos());
        }
        theta = theta.clamp(-90.0, 90.0);
        let st = r2d(d2r(theta).cos().clamp(-1.0, 1.0).acos());
        let mut delta = if st == 0.0 {
            90.0
        } else {
            r2d((d2r(theta).sin() / d2r(st).sin()).clamp(-1.0, 1.0).asin())
        };
        if theta.abs() >= 90.0 {
            delta = -delta + theta.signum() * 180.0;
        }
        (st, (delta + 360.0) % 360.0)
    };
    let (z, t, da) = (d2r(zenith), d2r(tilt), d2r(sun_azimuth - azimuth));
    let cos_aoi = z.sin() * t.sin() * da.cos() + z.cos() * t.cos();
    let (dni, ghi, dhi) = (row.dni.max(0.0), row.ghi.max(0.0), row.dhi.max(0.0));
    let b = 2.0 * std::f64::consts::PI * (day as f64 - 1.0) / 365.0;
    let dni_extra = 1366.1
        * (1.00011
            + 0.034221 * b.cos()
            + 0.00128 * b.sin()
            + 0.000719 * (2.0 * b).cos()
            + 0.000077 * (2.0 * b).sin());
    let anisotropy = dni / dni_extra;
    let rb = cos_aoi.max(0.0) / z.cos().max(0.01745);
    let sky = (dhi * (1.0 - anisotropy) * 0.5 * (1.0 + t.cos())).max(0.0)
        + (dhi * anisotropy * rb).max(0.0);
    let ground = ghi * 0.25 * (1.0 - t.cos()) / 2.0;
    let beam = (dni * cos_aoi).max(0.0);
    let poa = beam + sky + ground;
    let aoi = r2d(cos_aoi.clamp(-1.0, 1.0).acos());
    let n1 = 1.0;
    let n2 = 1.526;
    let s1 = d2r(aoi).sin();
    let c1 = d2r(aoi).cos();
    let s2 = s1 / n2;
    let c2 = (1.0 - s2 * s2).sqrt();
    let rs = ((n1 * c1 - n2 * c2) / (n1 * c1 + n2 * c2)).powi(2);
    let rp = ((n1 * c2 - n2 * c1) / (n1 * c2 + n2 * c1)).powi(2);
    let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
    let iam = (((1.0 - rs) + (1.0 - rp)) / 2.0 * (-4.0_f64 * 0.002 / c2).exp())
        / ((1.0 - r0) * (-4.0_f64 * 0.002).exp());
    let effective = beam * iam + sky + ground;
    let temp_module = poa * (-3.56 - 0.075 * row.wind).exp() + row.temp_air;
    let temp_cell = temp_module + poa / 1000.0 * 3.0;
    let pdc = effective / 1000.0 * (1.0 - 0.004 * (temp_cell - 25.0));
    if pdc <= 0.0 {
        return 0.0;
    }
    let eta = 0.96 / 0.9637 * (-0.0162 * pdc - 0.0059 / pdc + 0.9858);
    (eta * pdc).clamp(0.0, 0.96)
}

fn weather_profiles(
    weather: &[WeatherRow],
    latitude: f64,
    longitude: f64,
    fixed_tilt: f64,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let location = Location {
        latitude,
        longitude,
    };
    let spa = SolarPositions::new();
    let atmosphere = RefractionCorrection::new(1013.25, 12.0).ok();
    let (tilt, azimuth) = if fixed_tilt >= 0.0 {
        (fixed_tilt, if latitude >= 0.0 { 180.0 } else { 0.0 })
    } else {
        fixed_orientation(latitude)
    };
    let mut tracking = Vec::with_capacity(weather.len());
    let mut fixed = Vec::with_capacity(weather.len());
    for row in weather {
        let dt = NaiveDateTime::parse_from_str(&row.time_utc, "%Y%m%d:%H%M")
            .map_err(|e| e.to_string())?;
        let time = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
        let position = spa
            .at(&time, location, 0.0, 67.0, atmosphere)
            .map_err(|e| e.to_string())?;
        let zenith = position.zenith_angle();
        let sun_az = position.azimuth();
        let day = dt.ordinal();
        tracking.push(plane_ac(row, zenith, sun_az, day, false, tilt, azimuth));
        fixed.push(plane_ac(row, zenith, sun_az, day, true, tilt, azimuth));
    }
    Ok((tracking, fixed))
}

#[derive(Clone, Serialize)]
struct ResultRow {
    configuration: String,
    tilt: Option<f64>,
    solar_mw: i32,
    bess_mw: i32,
    generator_mw: f64,
    pv_yield_mwh_per_mw: f64,
    average_generator_mwh: f64,
    renewable_pct: f64,
    load_served_pct: f64,
    lcoe: f64,
    coarse_pairs: usize,
    refined_pairs: usize,
}

#[derive(Serialize)]
struct Output {
    selected: ResultRow,
    comparison: Vec<ResultRow>,
}

#[derive(Clone, Copy)]
enum DispatchStep {
    Dark(usize),
    Solar(f64),
}

fn dispatch_timeline(profile: &[f64]) -> Vec<DispatchStep> {
    let mut steps = Vec::with_capacity(profile.len());
    let mut i = 0;
    while i < profile.len() {
        if profile[i] == 0.0 {
            let start = i;
            while i < profile.len() && profile[i] == 0.0 {
                i += 1;
            }
            steps.push(DispatchStep::Dark(i - start));
        } else {
            steps.push(DispatchStep::Solar(profile[i]));
            i += 1;
        }
    }
    steps
}

fn dispatch(
    profile: &[f64],
    pv: i32,
    bess: i32,
    load: f64,
    generator: f64,
) -> ([f64; YEARS], [f64; YEARS]) {
    dispatch_steps(&dispatch_timeline(profile), pv, bess, load, generator)
}

fn dispatch_steps(
    profile: &[DispatchStep],
    pv: i32,
    bess: i32,
    load: f64,
    generator: f64,
) -> ([f64; YEARS], [f64; YEARS]) {
    let energy = bess as f64 * BATTERY_HOURS;
    let mut gens = [0.0; YEARS];
    let mut served = [0.0; YEARS];
    for year in 1..=YEARS {
        let max_state = energy * (1.0 - BESS_DEG * (year - 1) as f64);
        let pv_factor = pv as f64 / DC_AC * (1.0 - PV_DEG * (year - 1) as f64);
        let mut state = energy; // The source model initializes every operating year full.
        let (mut gen_total, mut unmet_total) = (0.0, 0.0);
        for step in profile {
            match step {
                DispatchStep::Solar(p) => {
                    let balance = p * pv_factor - load;
                    if balance > 0.0 {
                        let stored = balance.min(bess as f64).min(max_state - state);
                        state += stored * SQRT_RTE;
                    } else {
                        let deficit = -balance;
                        let discharge = (deficit / SQRT_RTE).min(state).min(bess as f64);
                        let delivered = discharge * SQRT_RTE;
                        let remainder = deficit - delivered;
                        gen_total += remainder.min(generator);
                        unmet_total += (remainder - generator).max(0.0);
                        state -= discharge;
                    }
                }
                DispatchStep::Dark(hours) => {
                    let hours = *hours;
                    let max_discharge = (load / SQRT_RTE).min(bess as f64);
                    if hours == 0 || max_discharge == 0.0 || state == 0.0 {
                        gen_total += hours as f64 * load.min(generator);
                        unmet_total += hours as f64 * (load - generator).max(0.0);
                    } else {
                        let full_hours = if state >= max_discharge * hours as f64 {
                            hours
                        } else {
                            (state / max_discharge).floor() as usize
                        };
                        let remainder_state = (state - full_hours as f64 * max_discharge).max(0.0);
                        let full_deficit = load - max_discharge * SQRT_RTE;
                        gen_total += full_hours as f64 * full_deficit.min(generator);
                        unmet_total += full_hours as f64 * (full_deficit - generator).max(0.0);
                        let mut used_hours = full_hours;
                        if full_hours < hours && remainder_state > 0.0 {
                            let delivered = remainder_state * SQRT_RTE;
                            let deficit = load - delivered;
                            gen_total += deficit.min(generator);
                            unmet_total += (deficit - generator).max(0.0);
                            state = 0.0;
                            used_hours += 1;
                        } else {
                            state = remainder_state;
                        }
                        let empty_hours = hours - used_hours;
                        gen_total += empty_hours as f64 * load.min(generator);
                        unmet_total += empty_hours as f64 * (load - generator).max(0.0);
                    }
                }
            }
        }
        gens[year - 1] = gen_total.round_ties_even();
        served[year - 1] = (load * 8760.0 - unmet_total).round_ties_even();
    }
    (gens, served)
}

fn lcoe(
    gens: &[f64; YEARS],
    served: &[f64; YEARS],
    pv: i32,
    bess: i32,
    gen: f64,
    c: &Inputs,
) -> f64 {
    let solar_capex = c.pv_cost * pv as f64;
    let bess_capex = c.bess_cost * bess as f64 * BATTERY_HOURS / 1000.0;
    let gen_capex = c.gen_capex * gen / 1000.0;
    let integration = c.integration * c.load / 1000.0;
    let hard = solar_capex + bess_capex + gen_capex + integration;
    let total = hard * (1.0 + c.soft_pct / 100.0);
    let leverage = c.leverage / 100.0;
    let debt = total * leverage;
    let rate = c.debt_cost / 100.0;
    let n = c.debt_term.max(1) as i32;
    let payment = if rate == 0.0 {
        debt / n as f64
    } else {
        let growth = (1.0 + rate).powi(n);
        debt * rate * growth / (growth - 1.0)
    };
    let renewable_share = if hard == 0.0 {
        0.0
    } else {
        (solar_capex + bess_capex) / hard
    };
    let itc = total * renewable_share * c.itc / 100.0;
    let depreciable = total - itc / 2.0;
    let construction = c.construction_years.max(1) as i32;
    let equity_rate = c.equity_cost / 100.0;
    let mut npv0 = 0.0;
    for year in (-construction + 1)..=0 {
        let period = (year + construction) as f64;
        npv0 +=
            -(total / construction as f64) * (1.0 - leverage) / (1.0 + equity_rate).powf(period);
    }
    let mut price_derivative = 0.0;
    let mut outstanding = Some(debt);
    let heat = if c.gen_type == "Gas Engine" {
        8989.0
    } else {
        9630.0
    };
    for i in 0..YEARS {
        let year = i + 1;
        let period = (year + construction as usize) as f64;
        let discount = (1.0 + equity_rate).powf(period);
        let escalation = (1.0 + c.om_escalator / 100.0).powi(i as i32);
        let fuel_escalation = (1.0 + c.fuel_escalator / 100.0).powi(i as i32);
        let fuel = -c.fuel_price * fuel_escalation * gens[i] * heat / 1000.0 / 1_000_000.0;
        let fixed = -(c.solar_om * pv as f64
            + c.bess_om * bess as f64
            + c.gen_om * gen
            + c.bos_om * c.load)
            / 1000.0
            * escalation
            - c.soft_om * escalation / 100.0 * hard;
        let variable = -c.gen_var * escalation * gens[i] / 1000.0;
        let op = fuel + fixed + variable;
        let interest = outstanding.map(|value| -value * rate);
        let debt_service = -payment;
        let depreciation = -c.depreciation.get(i).copied().unwrap_or(0.0) / 100.0 * depreciable;
        let (after_zero, price_factor) = if let Some(interest) = interest {
            let taxable = op + depreciation + interest;
            let credit = if year == 1 { itc } else { 0.0 };
            (
                op + debt_service - taxable * c.tax_rate / 100.0 + credit,
                1.0 - c.tax_rate / 100.0,
            )
        } else {
            (op + debt_service, 1.0)
        };
        npv0 += after_zero / discount;
        price_derivative += served[i] / 1_000_000.0 * price_factor / discount;
        if year < c.debt_term {
            if let Some(balance) = outstanding {
                let principal = debt_service - interest.unwrap_or(0.0);
                outstanding = Some(balance + principal);
            }
        } else {
            outstanding = None;
        }
    }
    if price_derivative <= 0.0 {
        f64::INFINITY
    } else {
        -npv0 / price_derivative
    }
}

fn renewable_pct(gens: &[f64; YEARS], served: &[f64; YEARS]) -> f64 {
    100.0 * (1.0 - gens.iter().sum::<f64>() / served.iter().sum::<f64>().max(1.0))
}

fn profile_result(
    profile: &[f64],
    c: &Inputs,
    configuration: &str,
    tilt: Option<f64>,
) -> ResultRow {
    let standalone = c.standalone;
    let optimizing = c.mode == "optimize";
    let mut sizing_inputs = c.clone();
    if optimizing {
        sizing_inputs.generator = c.load;
    }
    let generator = if standalone {
        0.0
    } else if optimizing {
        c.load
    } else {
        c.generator
    };
    let (solar, bess, coarse_pairs, refined_pairs) = if optimizing {
        optimize(profile, &sizing_inputs, c.target)
    } else {
        (c.solar, c.bess, 0, 0)
    };
    // Standalone cases optimize with the turbine in place, then remove its capacity and costs.
    let (design_gen, design_served) = dispatch(
        profile,
        solar,
        bess,
        c.load,
        if optimizing { c.load } else { c.generator },
    );
    let renewable = renewable_pct(&design_gen, &design_served);
    let (reported_gen, reported_served) = if standalone {
        dispatch(profile, solar, bess, c.load, 0.0)
    } else {
        (design_gen, design_served)
    };
    let mut costs = c.clone();
    if standalone {
        costs.gen_capex = 0.0;
        costs.gen_om = 0.0;
        costs.gen_var = 0.0;
    }
    let result_lcoe = lcoe(
        &reported_gen,
        &reported_served,
        solar,
        bess,
        generator,
        &costs,
    );
    let served_pct = 100.0 * reported_served.iter().sum::<f64>() / (c.load * 8760.0 * YEARS as f64);
    ResultRow {
        configuration: configuration.into(),
        tilt,
        solar_mw: solar,
        bess_mw: bess,
        generator_mw: generator,
        pv_yield_mwh_per_mw: profile.iter().sum(),
        average_generator_mwh: reported_gen.iter().sum::<f64>() / YEARS as f64,
        renewable_pct: renewable,
        load_served_pct: served_pct,
        lcoe: result_lcoe,
        coarse_pairs,
        refined_pairs,
    }
}

fn limits(load: f64) -> (i32, i32, i32) {
    let solar = 10_000i32.max(((load * 150.0 / 100.0).ceil() * 100.0) as i32);
    let bess = 10_000i32.max(((load * 500.0 / 100.0).ceil() * 100.0) as i32);
    let step = 1000i32.max((((bess as f64 / 250.0) / 100.0).ceil() * 100.0) as i32);
    (solar, bess, step)
}

fn candidate(profile: &[DispatchStep], c: &Inputs, pv: i32, bess: i32) -> (f64, f64) {
    let (gen, served) = dispatch_steps(profile, pv, bess, c.load, c.generator);
    let r = renewable_pct(&gen, &served);
    let price = lcoe(&gen, &served, pv, bess, c.generator, c);
    (r, price)
}

fn optimize(profile: &[f64], c: &Inputs, target: f64) -> (i32, i32, usize, usize) {
    let timeline = dispatch_timeline(profile);
    let (max_pv, max_bess, step) = limits(c.load);
    let solar_values = grid_values(max_pv, step);
    let bess_values = grid_values(max_bess, step);
    let mut best = (i32::MAX, i32::MAX, f64::INFINITY);
    let mut coarse = Vec::new();
    let mut ranked = Vec::new();
    for &pv in &solar_values {
        for &b in &bess_values {
            let (r, cost) = candidate(&timeline, c, pv, b);
            ranked.push((pv, b, r, cost));
            if r >= target {
                coarse.push((pv, b, cost));
                if cost < best.2 {
                    best = (pv, b, cost);
                }
            }
        }
    }
    let ncoarse = solar_values.len() * bess_values.len();
    // Fine search is bounded around well-spaced low-cost coarse candidates.
    coarse.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut seeds: Vec<(i32, i32)> = Vec::new();
    let ranked_by_value = if coarse.is_empty() {
        ranked.sort_by(|a, b| b.2.total_cmp(&a.2));
        ranked
            .iter()
            .map(|(p, b, r, _)| (*p, *b, *r))
            .collect::<Vec<_>>()
    } else {
        coarse
            .iter()
            .map(|(p, b, price)| (*p, *b, *price))
            .collect::<Vec<_>>()
    };
    for &(p, b, _) in &ranked_by_value {
        if seeds
            .iter()
            .all(|&(sp, sb)| (p - sp).abs().max((b - sb).abs()) >= step)
        {
            seeds.push((p, b));
            if seeds.len() == 12 {
                break;
            }
        }
    }
    let mut pairs = Vec::new();
    for (sp, sb) in seeds {
        let p0 = ((sp - step).max(0) / 100) * 100;
        let p1 = (sp + step).min(max_pv) / 100 * 100;
        let b0 = ((sb - step).max(0) / 100) * 100;
        let b1 = (sb + step).min(max_bess) / 100 * 100;
        let mut p = p0;
        while p <= p1 {
            let mut b = b0;
            while b <= b1 {
                pairs.push((p, b));
                b += 100;
            }
            p += 100;
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    for &(p, b) in &pairs {
        let (r, cost) = candidate(&timeline, c, p, b);
        if r >= target && cost < best.2 {
            best = (p, b, cost);
        }
    }
    if best.0 == i32::MAX {
        return (0, 0, ncoarse, pairs.len());
    }
    (best.0, best.1, ncoarse, pairs.len())
}

fn grid_values(maximum: i32, step: i32) -> Vec<i32> {
    let mut values: Vec<i32> = (0..=maximum).step_by(step as usize).collect();
    if values.last().copied() != Some(maximum) {
        values.push(maximum);
    }
    values
}

fn solve(c: Inputs) -> Result<Output, String> {
    let calculated_profiles = if c.weather.is_empty() {
        None
    } else {
        Some(weather_profiles(
            &c.weather,
            c.latitude,
            c.longitude,
            c.fixed_tilt,
        )?)
    };
    let tracking_profile = calculated_profiles
        .as_ref()
        .map(|p| p.0.as_slice())
        .unwrap_or(c.tracking.as_slice());
    let fixed_profile = calculated_profiles
        .as_ref()
        .map(|p| p.1.as_slice())
        .unwrap_or(c.fixed.as_slice());
    let mut profiles: Vec<(&str, &[f64], Option<f64>)> = Vec::new();
    if c.only != "fixed" && !tracking_profile.is_empty() {
        profiles.push(("Tracking array", tracking_profile, None));
    }
    if c.only != "tracking" && !fixed_profile.is_empty() {
        profiles.push(("Fixed array", fixed_profile, Some(c.fixed_tilt)));
    }
    if profiles.is_empty() && !c.profile.is_empty() {
        profiles.push(("Tracking", &c.profile, None));
    }
    if profiles.is_empty() {
        return Err("Missing hourly solar profile".into());
    }
    if c.mode == "optimize" && !(c.target == 95.0 || c.target == 99.0) {
        return Err("Target must be 95% or 99%".into());
    }
    let mut comparison: Vec<ResultRow> = profiles
        .iter()
        .map(|(label, p, tilt)| profile_result(p, &c, label, *tilt))
        .collect();
    if comparison.iter().any(|r| !r.lcoe.is_finite()) {
        return Err("No energy served; cannot calculate LCOE".into());
    }
    comparison.sort_by(|a, b| a.lcoe.total_cmp(&b.lcoe));
    let selected = comparison[0].clone();
    Ok(Output {
        selected,
        comparison,
    })
}

#[wasm_bindgen]
pub fn calculate(value: JsValue) -> Result<JsValue, JsValue> {
    let inputs: Inputs =
        serde_wasm_bindgen::from_value(value).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let output = solve(inputs).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&output).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_dispatch(
        profile: &[f64],
        pv: i32,
        bess: i32,
        load: f64,
        generator: f64,
    ) -> ([f64; YEARS], [f64; YEARS]) {
        let energy = bess as f64 * BATTERY_HOURS;
        let mut gens = [0.0; YEARS];
        let mut served = [0.0; YEARS];
        for year in 1..=YEARS {
            let max_state = energy * (1.0 - BESS_DEG * (year - 1) as f64);
            let factor = pv as f64 / DC_AC * (1.0 - PV_DEG * (year - 1) as f64);
            let mut state = energy;
            let (mut gen_total, mut unmet_total) = (0.0, 0.0);
            for &p in profile {
                let balance = p * factor - load;
                if balance > 0.0 {
                    let stored = balance.min(bess as f64).min(max_state - state);
                    state += stored * SQRT_RTE;
                } else {
                    let deficit = -balance;
                    let discharge = (deficit / SQRT_RTE).min(state).min(bess as f64);
                    let delivered = discharge * SQRT_RTE;
                    let remainder = deficit - delivered;
                    gen_total += remainder.min(generator);
                    unmet_total += (remainder - generator).max(0.0);
                    state -= discharge;
                }
            }
            gens[year - 1] = gen_total.round_ties_even();
            served[year - 1] = (load * 8760.0 - unmet_total).round_ties_even();
        }
        (gens, served)
    }

    #[test]
    fn defaults_are_500_mw_generator() {
        let c: Inputs = serde_json::from_str("{}").unwrap();
        assert_eq!(c.generator, 500.0);
    }
    #[test]
    fn annualized_dispatch_matches_source_rules() {
        let p = vec![0.0; 8760];
        let (g, s) = dispatch(&p, 0, 0, 100.0, 100.0);
        assert_eq!(g[0], 876000.0);
        assert_eq!(s[0], 876000.0);
    }

    #[test]
    fn compressed_dark_hours_match_hourly_dispatch() {
        let mut profile = vec![0.0; 8760];
        for day in 0..365 {
            for hour in 7..17 {
                profile[day * 24 + hour] = match hour % 4 {
                    0 => 0.12,
                    1 => 0.58,
                    2 => 1.35,
                    _ => 0.77,
                };
            }
        }
        for (pv, bess, load, generator) in [
            (500, 500, 100.0, 100.0),
            (1400, 700, 100.0, 43.5),
            (900, 0, 100.0, 100.0),
            (2000, 1200, 350.0, 350.0),
        ] {
            assert_eq!(
                dispatch(&profile, pv, bess, load, generator),
                reference_dispatch(&profile, pv, bess, load, generator),
                "pv={pv}, bess={bess}, load={load}, generator={generator}"
            );
        }
    }
}
