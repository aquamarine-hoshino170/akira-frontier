let gamma = rng.gen_range(0.1..0.5);
let omega = rng.gen_range(1.0..5.0);
let alpha = rng.gen_range(0.5..2.0);

let eq_x = format!("{}*y - {}*x + {}*x*z", omega, gamma, alpha);
let eq_y = format!("-{}*x - {}*y + {}*y*z", omega, gamma, alpha);
let eq_z = format!("-{}*(z - 1.0) - {}*(x*x + y*y)", gamma, alpha);
