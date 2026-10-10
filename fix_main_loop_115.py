with open("src/main.rs", "r", encoding="utf-8") as f:
    text = f.read()

old_loop_target = """        loop {
            let (f, state, params, fam) = match rng.gen_range(0u8..15) {
                0 => { let (f, s, p) = gen_sprott(&mut rng);          (f, s, p, "sprott-3D") }
                1 => { let (f, s, p) = gen_oscillator(&mut rng);      (f, s, p, "oscillator") }
                2 => { let (f, s, p) = gen_1d_auto(&mut rng);        (f, s, p, "autonomous-1D") }
                3 => { let (f, s, p) = gen_lienard(&mut rng);        (f, s, p, "lienard-2D") }
                4 => { let (f, s, p) = gen_biology(&mut rng);        (f, s, p, "biology-FHN") }
                5 => { let (f, s, p) = gen_chemistry(&mut rng);      (f, s, p, "chem-brusselator") }
                6 => { let (f, s, p) = gen_cosmology(&mut rng);      (f, s, p, "cosmo-quintessence") }
                7 => { let (f, s, p) = gen_quantum(&mut rng);        (f, s, p, "quantum-semiclass") }
                8 => { let (f, s, p) = gen_oregonator_3d(&mut rng);  (f, s, p, "chem-oregonator-3D") }
                9 => { let (f, s, p) = gen_foodweb_3d(&mut rng);     (f, s, p, "bio-foodweb-3D") }
                10 => { let (f, s, p) = gen_fluid_navier_stokes(&mut rng); (f, s, p, "fluid-navier-stokes") }
                11 => { let (f, s, p) = gen_maxwell_bloch(&mut rng);       (f, s, p, "em-maxwell-bloch") }
                12 => { let (f, s, p) = gen_general_relativity(&mut rng);   (f, s, p, "gr-mixmaster") }
                13 => { let (f, s, p) = gen_thermo_dissipative(&mut rng);   (f, s, p, "thermo-onsager") }
                _ => { let (f, s, p) = gen_yang_mills_gauge(&mut rng);     (f, s, p, "gauge-yang-mills") }
            };"""

new_loop_target = """        loop {
            let (f, state, params, fam) = match rng.gen_range(0u16..115) {
                0 => { let (f, s, p) = gen_sprott(&mut rng);               (f, s, p, "sprott-3D") }
                1 => { let (f, s, p) = gen_oscillator(&mut rng);           (f, s, p, "oscillator") }
                2 => { let (f, s, p) = gen_1d_auto(&mut rng);             (f, s, p, "autonomous-1D") }
                3 => { let (f, s, p) = gen_lienard(&mut rng);             (f, s, p, "lienard-2D") }
                4 => { let (f, s, p) = gen_biology(&mut rng);             (f, s, p, "biology-FHN") }
                5 => { let (f, s, p) = gen_chemistry(&mut rng);           (f, s, p, "chem-brusselator") }
                6 => { let (f, s, p) = gen_cosmology(&mut rng);           (f, s, p, "cosmo-quintessence") }
                7 => { let (f, s, p) = gen_quantum(&mut rng);             (f, s, p, "quantum-semiclass") }
                8 => { let (f, s, p) = gen_oregonator_3d(&mut rng);       (f, s, p, "chem-oregonator-3D") }
                9 => { let (f, s, p) = gen_foodweb_3d(&mut rng);          (f, s, p, "bio-foodweb-3D") }
                10 => { let (f, s, p) = gen_fluid_navier_stokes(&mut rng); (f, s, p, "fluid-navier-stokes") }
                11 => { let (f, s, p) = gen_maxwell_bloch(&mut rng);       (f, s, p, "em-maxwell-bloch") }
                12 => { let (f, s, p) = gen_general_relativity(&mut rng);   (f, s, p, "gr-mixmaster") }
                13 => { let (f, s, p) = gen_thermo_dissipative(&mut rng);   (f, s, p, "thermo-onsager") }
                14 => { let (f, s, p) = gen_yang_mills_gauge(&mut rng);     (f, s, p, "gauge-yang-mills") }
                other => {
                    let ext_id = (other - 15) as usize;
                    gen_extended_100_domain(&mut rng, ext_id)
                }
            };"""

if old_loop_target in text:
    text = text.replace(old_loop_target, new_loop_target)
    print("SUCCESS: main() loop updated to 115 domains!")
else:
    # যদি স্পেসিং সামান্য অমিল থাকে, রেজেক্স দিয়ে রিপ্লেস করা
    import re
    pattern = r"loop\s*\{\s*let\s*\(f,\s*state,\s*params,\s*fam\)\s*=\s*match\s*rng\.gen_range\(0u8\.\.15\)\s*\{[\s\S]*?gen_yang_mills_gauge\(&mut rng\);\s*\(f,\s*s,\s*p,\s*\"gauge-yang-mills\"\)\s*\}\s*\};"
    text = re.sub(pattern, new_loop_target.strip(), text, count=1)
    print("SUCCESS: main() loop updated via regex!")

with open("src/main.rs", "w", encoding="utf-8") as f:
    f.write(text)
