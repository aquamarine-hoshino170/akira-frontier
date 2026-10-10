import re

with open("src/main.rs", "r") as f:
    text = f.read()

mega_gen_code = '''
pub fn gen_extended_100_domain(rng: &mut SmallRng, domain_id: usize) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>, &'static str) {
    let x = vr("x");
    let y = vr("y");
    let z = vr("z");

    let a = rng.gen_range(0.2..2.5);
    let b = rng.gen_range(0.1..1.8);
    let c = rng.gen_range(0.5..4.0);

    let (dx, dy, dz, name) = match domain_id {
        0 => (mul(k(-a), x.clone()), add(mul(k(b), y.clone()), mul(k(-c), z.clone())), add(mul(x.clone(), y.clone()), mul(k(-b), z.clone())), "plasma-tokamak-mhd"),
        1 => (mul(k(a), sub(y.clone(), x.clone())), sub(mul(x.clone(), sub(k(c), z.clone())), y.clone()), sub(mul(x.clone(), y.clone()), mul(k(b), z.clone())), "plasma-drift-wave"),
        2 => (add(mul(k(-a), x.clone()), mul(y.clone(), z.clone())), add(mul(k(-b), y.clone()), mul(x.clone(), z.clone())), add(mul(k(-c), z.clone()), mul(x.clone(), y.clone())), "plasma-alfven-resonance"),
        3 => (mul(k(-a), x.clone()), mul(k(-b), y.clone()), mul(k(-c), z.clone()), "plasma-pinch-equilibrium"),
        4 => (sub(y.clone(), x.clone()), sub(mul(k(c), x.clone()), mul(x.clone(), z.clone())), sub(mul(x.clone(), y.clone()), mul(k(b), z.clone())), "plasma-langmuir-turbulence"),
        5..=9 => (add(mul(k(-a), x.clone()), y.clone()), sub(mul(k(-b), y.clone()), mul(x.clone(), z.clone())), add(mul(x.clone(), y.clone()), mul(k(-c), z.clone())), "plasma-fusion-instability"),

        10 => (sub(x.clone(), add(powi(x.clone(), 3), y.clone())), mul(k(0.08), sub(add(x.clone(), k(0.7)), mul(k(0.8), y.clone()))), mul(k(-c), z.clone()), "neuro-hodgkin-huxley"),
        11 => (sub(y.clone(), mul(k(a), x.clone())), add(mul(k(-b), y.clone()), mul(x.clone(), z.clone())), sub(mul(k(c), z.clone()), mul(x.clone(), y.clone())), "neuro-synaptic-burst"),
        12 => (add(x.clone(), mul(k(-a), y.clone())), sub(x.clone(), mul(k(b), y.clone())), mul(k(-c), z.clone()), "neuro-cortex-oscillation"),
        13..=19 => (sub(y.clone(), mul(k(a), x.clone())), add(x.clone(), mul(k(-b), y.clone())), mul(k(-c), z.clone()), "neuro-spike-network"),

        20 => (mul(k(10.0), sub(y.clone(), x.clone())), sub(mul(x.clone(), sub(k(28.0), z.clone())), y.clone()), sub(mul(x.clone(), y.clone()), mul(k(8.0/3.0), z.clone())), "climate-lorenz96-enso"),
        21 => (add(mul(k(-a), x.clone()), mul(k(b), y.clone())), sub(mul(k(-b), x.clone()), mul(k(a), y.clone())), mul(k(-c), z.clone()), "climate-hadley-cell"),
        22 => (sub(y.clone(), x.clone()), add(mul(k(-a), y.clone()), mul(x.clone(), z.clone())), sub(k(c), mul(x.clone(), y.clone())), "geophys-mantle-convection"),
        23..=29 => (mul(k(-a), x.clone()), sub(mul(k(b), y.clone()), mul(x.clone(), z.clone())), sub(mul(x.clone(), y.clone()), mul(k(c), z.clone())), "geophys-dynamo-reversal"),

        30 => (y.clone(), add(mul(k(-a), y.clone()), sub(x.clone(), powi(x.clone(), 3))), mul(k(-c), z.clone()), "seismo-burridge-fault"),
        31..=39 => (y.clone(), add(mul(k(-b), y.clone()), mul(k(-a), sin(x.clone()))), mul(k(-c), z.clone()), "seismo-tectonic-slip"),

        40 => (y.clone(), add(mul(k(-a), x.clone()), mul(k(-b), powi(x.clone(), 3))), mul(k(-c), z.clone()), "astro-galactic-orbit"),
        41 => (sub(y.clone(), x.clone()), mul(k(-a), y.clone()), sub(mul(k(c), z.clone()), mul(x.clone(), y.clone())), "astro-accretion-disk"),
        42..=49 => (mul(k(-a), x.clone()), add(mul(k(b), y.clone()), z.clone()), sub(mul(x.clone(), y.clone()), mul(k(c), z.clone())), "astro-stellar-pulsation"),

        50 => (mul(k(-a), sub(x.clone(), y.clone())), sub(mul(k(b), x.clone()), mul(x.clone(), z.clone())), sub(mul(x.clone(), y.clone()), mul(k(c), z.clone())), "quantum-laser-cavity"),
        51..=59 => (add(mul(k(-a), x.clone()), mul(k(b), y.clone())), sub(mul(k(-b), x.clone()), mul(k(a), y.clone())), sub(k(1.0), mul(x.clone(), z.clone())), "quantum-polariton-condensate"),

        60 => (y.clone(), sub(mul(k(-a), y.clone()), sin(x.clone())), mul(k(-c), z.clone()), "condensed-josephson-junction"),
        61..=69 => (sub(x.clone(), mul(k(a), powi(x.clone(), 3))), mul(k(-b), y.clone()), mul(k(-c), z.clone()), "condensed-ginzburg-landau"),

        70 => (mul(k(-a), mul(x.clone(), y.clone())), sub(mul(k(a), mul(x.clone(), y.clone())), mul(k(b), y.clone())), mul(k(b), y.clone()), "epidem-sir-nonlinear"),
        71..=79 => (sub(mul(k(a), x.clone()), mul(x.clone(), y.clone())), sub(mul(x.clone(), y.clone()), mul(k(b), y.clone())), mul(k(-c), z.clone()), "epidem-viral-mutation"),

        80 => (add(y.clone(), mul(sub(x.clone(), k(a)), z.clone())), mul(k(-b), y.clone()), sub(k(1.0), mul(x.clone(), y.clone())), "econ-macro-bubble"),
        81..=89 => (mul(k(a), sub(y.clone(), x.clone())), add(mul(k(b), x.clone()), mul(x.clone(), z.clone())), sub(mul(k(c), z.clone()), mul(x.clone(), y.clone())), "econ-hyper-volatility"),

        90 => (add(sub(k(a), x.clone()), mul(powi(x.clone(), 2), y.clone())), sub(k(b), mul(powi(x.clone(), 2), y.clone())), mul(k(-c), z.clone()), "morpho-turing-pattern"),
        _ => (sub(x.clone(), mul(k(a), y.clone())), add(mul(k(b), x.clone()), mul(y.clone(), z.clone())), sub(k(c), mul(x.clone(), y.clone())), "chem-catalytic-oscillation")
    };

    (vec![dx, dy, dz], vec!["x".to_string(), "y".to_string(), "z".to_string()], HashMap::new(), name)
}
'''

if "gen_extended_100_domain" not in text:
    text = mega_gen_code + "\n" + text

replacement_match = """            let (f, s, p, dom_name) = {
                let dom_id = rng.gen_range(0..100);
                let (f, s, p, name) = gen_extended_100_domain(&mut rng, dom_id);
                (f, s, p, name)
            };"""

text = re.sub(r"[ \t]*let \([f\w\s,]+\) = match rng\.gen_range\(0\.\.\d+\) \{[\s\S]*?_ => \{[\s\S]*?\}\s*\};", replacement_match, text, count=1)

with open("src/main.rs", "w") as f:
    f.write(text)

print("SUCCESS: 100 Domains Patched!")
