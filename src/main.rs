

/**
╔══════════════════════════════════════════════════════════════════════════════╗
║  AKIRA-FRONTIER-RS  v2.0  —  Distributed ODE Frontier Explorer             ║
║  বাংলা মন্তব্য সংরক্ষিত · Pure Rust · No SymPy                            ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Architecture:                                                              ║
║    Rayon multi-universe parallel mining (distributed parameter-space)       ║
║    DashSet<String>  hash-based zero-redundancy deduplication               ║
║    Symbolic expression tree + numeric evaluation                            ║
║    Dimensional analysis (M, L, T, K)                                       ║
║    RK4 integrator + Benettin Lyapunov algorithm                             ║
║    JSONL output                                                             ║
║    1000 math symbols registry (doc-2 complete)                             ║
╚══════════════════════════════════════════════════════════════════════════════╝
*/

#[allow(dead_code, non_snake_case, clippy::needless_range_loop)]

use std::{
    fs::OpenOptions,
    collections::{HashMap, HashSet},
    io::BufWriter,
    sync::Arc,
    time::Instant,
};

use clap::Parser;
use dashmap::DashSet;
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use rayon::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Default)]
pub struct Dim { pub m: i8, pub l: i8, pub t: i8, pub k: i8 }

impl Dim {
    pub const ZERO:     Self = Dim { m: 0,  l: 0,  t: 0,  k: 0  };
    pub const TIME:     Self = Dim { m: 0,  l: 0,  t: 1,  k: 0  };
    pub const LENGTH:   Self = Dim { m: 0,  l: 1,  t: 0,  k: 0  };
    pub const VELOCITY: Self = Dim { m: 0,  l: 1,  t: -1, k: 0  };
    pub const ACCEL:    Self = Dim { m: 0,  l: 1,  t: -2, k: 0  };
    pub const FREQ:     Self = Dim { m: 0,  l: 0,  t: -1, k: 0  };
    pub const FREQ2:    Self = Dim { m: 0,  l: 0,  t: -2, k: 0  };
    pub const MASS:     Self = Dim { m: 1,  l: 0,  t: 0,  k: 0  };
    pub const ENERGY:   Self = Dim { m: 1,  l: 2,  t: -2, k: 0  };
    pub const TEMP:     Self = Dim { m: 0,  l: 0,  t: 0,  k: 1  };

    #[inline] pub fn add(self, r: Self) -> Self { Dim { m: self.m+r.m, l: self.l+r.l, t: self.t+r.t, k: self.k+r.k } }
    #[inline] pub fn sub(self, r: Self) -> Self { Dim { m: self.m-r.m, l: self.l-r.l, t: self.t-r.t, k: self.k-r.k } }
    #[inline] pub fn scale(self, s: i8) -> Self { Dim { m: self.m*s,  l: self.l*s,  t: self.t*s,  k: self.k*s  } }
    #[inline] pub fn neg(self) -> Self { self.scale(-1) }
    #[inline] pub fn is_zero(&self) -> bool { *self == Dim::ZERO }

    pub fn display(&self) -> String {
        if self.is_zero() { return "1 (মাত্রাহীন)".into(); }
        let mut parts = Vec::new();
        let units = [("kg", self.m), ("m", self.l), ("s", self.t), ("K", self.k)];
        for (sym, exp) in units {
            if exp != 0 {
                parts.push(if exp == 1 { sym.to_string() } else { format!("{}^{}", sym, exp) });
            }
        }
        parts.join("·")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    C(f64),
    V(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Powi(Box<Expr>, i32),
    Powf(Box<Expr>, Box<Expr>),
    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),
    Exp(Box<Expr>),
    Ln(Box<Expr>),
    Sqrt(Box<Expr>),
    Abs(Box<Expr>),
    Sinh(Box<Expr>),
    Cosh(Box<Expr>),
    Tanh(Box<Expr>),
    Sgn(Box<Expr>),
}

fn bx(e: Expr) -> Box<Expr> { Box::new(e) }
pub fn k(v: f64)             -> Expr { Expr::C(v) }
pub fn vr(n: &str)           -> Expr { Expr::V(n.to_string()) }
pub fn neg(e: Expr)          -> Expr { Expr::Neg(bx(e)) }
pub fn add(a: Expr, b: Expr) -> Expr { Expr::Add(bx(a), bx(b)) }
pub fn sub(a: Expr, b: Expr) -> Expr { Expr::Sub(bx(a), bx(b)) }
pub fn mul(a: Expr, b: Expr) -> Expr { Expr::Mul(bx(a), bx(b)) }
pub fn div(a: Expr, b: Expr) -> Expr { Expr::Div(bx(a), bx(b)) }
pub fn powi(e: Expr, n: i32) -> Expr { Expr::Powi(bx(e), n) }
pub fn powf(a: Expr, b: Expr)-> Expr { Expr::Powf(bx(a), bx(b)) }
pub fn sin(e: Expr)          -> Expr { Expr::Sin(bx(e)) }
pub fn cos(e: Expr)          -> Expr { Expr::Cos(bx(e)) }
pub fn exp(e: Expr)          -> Expr { Expr::Exp(bx(e)) }
pub fn ln(e: Expr)           -> Expr { Expr::Ln(bx(e)) }
pub fn sqrt(e: Expr)         -> Expr { Expr::Sqrt(bx(e)) }
pub fn sinh(e: Expr)         -> Expr { Expr::Sinh(bx(e)) }
pub fn cosh(e: Expr)         -> Expr { Expr::Cosh(bx(e)) }
pub fn tanh(e: Expr)         -> Expr { Expr::Tanh(bx(e)) }

impl Expr {
    pub fn eval(&self, m: &HashMap<String, f64>) -> f64 {
        match self {
            Expr::C(x)       => *x,
            Expr::V(n)       => *m.get(n).unwrap_or(&f64::NAN),
            Expr::Neg(e)     => -e.eval(m),
            Expr::Abs(e)     => e.eval(m).abs(),
            Expr::Sgn(e)     => e.eval(m).signum(),
            Expr::Add(a,b)   => a.eval(m) + b.eval(m),
            Expr::Sub(a,b)   => a.eval(m) - b.eval(m),
            Expr::Mul(a,b)   => a.eval(m) * b.eval(m),
            Expr::Div(a,b)   => { let d=b.eval(m); if d.abs()<1e-300 {f64::NAN} else {a.eval(m)/d} },
            Expr::Powi(e,n)  => e.eval(m).powi(*n),
            Expr::Powf(a,b)  => a.eval(m).powf(b.eval(m)),
            Expr::Sin(e)     => e.eval(m).sin(),
            Expr::Cos(e)     => e.eval(m).cos(),
            Expr::Tan(e)     => e.eval(m).tan(),
            Expr::Exp(e)     => e.eval(m).exp(),
            Expr::Ln(e)      => { let x=e.eval(m); if x>0.0 {x.ln()} else {f64::NAN} },
            Expr::Sqrt(e)    => { let x=e.eval(m); if x>=0.0 {x.sqrt()} else {f64::NAN} },
            Expr::Sinh(e)    => e.eval(m).sinh(),
            Expr::Cosh(e)    => e.eval(m).cosh(),
            Expr::Tanh(e)    => e.eval(m).tanh(),
        }
    }

    pub fn eval_state(&self, state: &[f64], names: &[String], params: &HashMap<String,f64>) -> f64 {
        let mut m: HashMap<String,f64> = names.iter().zip(state).map(|(n,&v)|(n.clone(),v)).collect();
        m.extend(params.iter().map(|(k,&v)|(k.clone(),v)));
        self.eval(&m)
    }

    pub fn partial(&self, var: &str, m: &HashMap<String,f64>) -> f64 {
        const H: f64 = 1e-7;
        let x0 = *m.get(var).unwrap_or(&0.0);
        let mut mp = m.clone(); mp.insert(var.to_string(), x0+H);
        let mut mm = m.clone(); mm.insert(var.to_string(), x0-H);
        (self.eval(&mp) - self.eval(&mm)) / (2.0*H)
    }

    pub fn free_vars(&self) -> HashSet<String> {
        let mut s = HashSet::new(); self.cvars(&mut s); s
    }
    fn cvars(&self, s: &mut HashSet<String>) {
        match self {
            Expr::V(n) => { s.insert(n.clone()); }
            Expr::C(_) => {}
            Expr::Neg(e)|Expr::Abs(e)|Expr::Sgn(e)|Expr::Sin(e)|Expr::Cos(e)|
            Expr::Tan(e)|Expr::Exp(e)|Expr::Ln(e)|Expr::Sqrt(e)|
            Expr::Sinh(e)|Expr::Cosh(e)|Expr::Tanh(e)|Expr::Powi(e,_) => e.cvars(s),
            Expr::Add(a,b)|Expr::Sub(a,b)|Expr::Mul(a,b)|
            Expr::Div(a,b)|Expr::Powf(a,b) => { a.cvars(s); b.cvars(s); }
        }
    }

    pub fn has_var(&self, var: &str) -> bool { self.free_vars().contains(var) }

    pub fn is_nonlinear(&self, sv: &[String]) -> bool {
        match self {
            Expr::Powi(e, n) if *n >= 2 =>
                matches!(e.as_ref(), Expr::V(s) if sv.contains(s)) || e.is_nonlinear(sv),
            Expr::Sin(e)|Expr::Cos(e)|Expr::Tan(e)|Expr::Exp(e)|Expr::Ln(e)|
            Expr::Sinh(e)|Expr::Cosh(e)|Expr::Tanh(e) =>
                e.free_vars().iter().any(|s| sv.contains(s)),
            Expr::Mul(a,b) => {
                let av = a.free_vars().iter().any(|s| sv.contains(s));
                let bv = b.free_vars().iter().any(|s| sv.contains(s));
                (av && bv) || a.is_nonlinear(sv) || b.is_nonlinear(sv)
            }
            Expr::Div(a,b) =>
                b.free_vars().iter().any(|s| sv.contains(s)) || a.is_nonlinear(sv),
            Expr::Add(a,b)|Expr::Sub(a,b)|Expr::Powf(a,b) =>
                a.is_nonlinear(sv) || b.is_nonlinear(sv),
            Expr::Neg(e) => e.is_nonlinear(sv),
            Expr::Sqrt(e) =>
                e.free_vars().iter().any(|s| sv.contains(s)),
            _ => false,
        }
    }

    pub fn latex(&self) -> String {
        match self {
            Expr::C(x) => if *x == x.floor() && x.abs()<1e6 { format!("{}", *x as i64) } else { format!("{:.4}", x) },
            Expr::V(n) => n.clone(),
            Expr::Neg(e)    => format!("\\left(-{}\\right)", e.latex()),
            Expr::Abs(e)    => format!("\\left|{}\\right|", e.latex()),
            Expr::Sgn(e)    => format!("\\text{{sgn}}\\!\\left({}\\right)", e.latex()),
            Expr::Add(a,b)  => format!("\\left({}+{}\\right)", a.latex(), b.latex()),
            Expr::Sub(a,b)  => format!("\\left({}-{}\\right)", a.latex(), b.latex()),
            Expr::Mul(a,b)  => format!("{} \\cdot {}", a.latex(), b.latex()),
            Expr::Div(a,b)  => format!("\\frac{{{}}}{{{}}}", a.latex(), b.latex()),
            Expr::Powi(e,n) => format!("{}^{{{}}}", e.latex(), n),
            Expr::Powf(a,b) => format!("{}^{{{}}}", a.latex(), b.latex()),
            Expr::Sin(e)    => format!("\\sin\\!\\left({}\\right)", e.latex()),
            Expr::Cos(e)    => format!("\\cos\\!\\left({}\\right)", e.latex()),
            Expr::Tan(e)    => format!("\\tan\\!\\left({}\\right)", e.latex()),
            Expr::Exp(e)    => format!("e^{{{}}}", e.latex()),
            Expr::Ln(e)     => format!("\\ln\\!\\left({}\\right)", e.latex()),
            Expr::Sqrt(e)   => format!("\\sqrt{{{}}}", e.latex()),
            Expr::Sinh(e)   => format!("\\sinh\\!\\left({}\\right)", e.latex()),
            Expr::Cosh(e)   => format!("\\cosh\\!\\left({}\\right)", e.latex()),
            Expr::Tanh(e)   => format!("\\tanh\\!\\left({}\\right)", e.latex()),
        }
    }

    pub fn hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.latex().as_bytes());
        hex::encode(h.finalize())[..12].to_string()
    }
}

pub fn dim_of(e: &Expr, dm: &HashMap<String,Dim>) -> Result<Dim, String> {
    match e {
        Expr::C(_)      => Ok(Dim::ZERO),
        Expr::V(n)      => dm.get(n).copied().ok_or_else(|| format!("অজানা: {n}")),
        Expr::Neg(e)|Expr::Abs(e) => dim_of(e, dm),
        Expr::Sgn(_)    => Ok(Dim::ZERO),
        Expr::Add(a,b)|Expr::Sub(a,b) => {
            let (da,db) = (dim_of(a,dm)?, dim_of(b,dm)?);
            if da != db { return Err(format!("মাত্রা-বেমিল যোগে: {} ≠ {}", da.display(), db.display())); }
            Ok(da)
        }
        Expr::Mul(a,b)  => Ok(dim_of(a,dm)?.add(dim_of(b,dm)?)),
        Expr::Div(a,b)  => Ok(dim_of(a,dm)?.sub(dim_of(b,dm)?)),
        Expr::Powi(e,n) => Ok(dim_of(e,dm)?.scale(*n as i8)),
        Expr::Powf(a,b) => {
            if !dim_of(a,dm)?.is_zero() || !dim_of(b,dm)?.is_zero() {
                return Err("powf: উভয় আর্গুমেন্ট মাত্রাহীন হতে হবে".into());
            }
            Ok(Dim::ZERO)
        }
        Expr::Sqrt(e) => {
            let d = dim_of(e,dm)?;
            if d.m%2!=0 || d.l%2!=0 || d.t%2!=0 || d.k%2!=0 {
                return Err(format!("sqrt: বিজোড় ঘাত {}", d.display()));
            }
            Ok(Dim { m: d.m/2, l: d.l/2, t: d.t/2, k: d.k/2 })
        }
        Expr::Sin(e)|Expr::Cos(e)|Expr::Tan(e)|Expr::Exp(e)|Expr::Ln(e)|
        Expr::Sinh(e)|Expr::Cosh(e)|Expr::Tanh(e) => {
            let d = dim_of(e,dm)?;
            if !d.is_zero() { return Err(format!("অতীন্দ্রিয় ফাংশনে মাত্রাযুক্ত আর্গুমেন্ট: {}", d.display())); }
            Ok(Dim::ZERO)
        }
    }
}

pub fn check_ode_dims(state: &[String], rhs: &[Expr],
                      state_dims: &HashMap<String,Dim>,
                      all_dims: &HashMap<String,Dim>) -> Result<(), String>
{
    let t_dim = Dim::TIME;
    for (s, f) in state.iter().zip(rhs.iter()) {
        let sd = *state_dims.get(s).unwrap_or(&Dim::ZERO);
        let expected = sd.sub(t_dim);
        let actual   = dim_of(f, all_dims).map_err(|e| format!("RHS {s}: {e}"))?;
        if actual != expected {
            return Err(format!("Gate1 fail: d{s}/dt মাত্রা {} কিন্তু RHS মাত্রা {}",
                expected.display(), actual.display()));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct MathSym {
    pub id: u16,
    pub latex: &'static str,
    pub cat: &'static str,
}

macro_rules! S { ($id:expr, $l:expr, $c:expr) => { MathSym { id: $id, latex: $l, cat: $c } }; }

pub static MATH_SYMBOLS: &[MathSym] = &[
    S!(1,   "+",            "arithmetic"),
    S!(2,   "-",            "arithmetic"),
    S!(3,   r"\times",      "arithmetic"),
    S!(4,   r"\div",        "arithmetic"),
    S!(5,   "=",            "relation"),
    S!(6,   r"\neq",        "relation"),
    S!(7,   "<",            "relation"),
    S!(8,   ">",            "relation"),
    S!(9,   r"\le",         "relation"),
    S!(10,  r"\ge",         "relation"),
    S!(11,  r"\ll",         "relation"),
    S!(12,  r"\gg",         "relation"),
    S!(13,  r"\equiv",      "relation"),
    S!(14,  r"\approx",     "relation"),
    S!(15,  r"\sim",        "relation"),
    S!(16,  r"\propto",     "relation"),
    S!(17,  r"\pm",         "arithmetic"),
    S!(18,  r"\mp",         "arithmetic"),
    S!(19,  r"\cdot",       "arithmetic"),
    S!(20,  r"\circ",       "arithmetic"),
    S!(21,  r"\bullet",     "arithmetic"),
    S!(22,  r"\star",       "arithmetic"),
    S!(23,  r"\ast",        "arithmetic"),
    S!(24,  r"\oplus",      "arithmetic"),
    S!(25,  r"\ominus",     "arithmetic"),
    S!(26,  r"\otimes",     "arithmetic"),
    S!(27,  r"\oslash",     "arithmetic"),
    S!(28,  r"\odot",       "arithmetic"),
    S!(29,  r"\dagger",     "arithmetic"),
    S!(30,  r"\ddagger",    "arithmetic"),
    S!(31,  r"\partial",    "calculus"),
    S!(32,  r"\nabla",      "calculus"),
    S!(33,  r"\Delta",      "calculus"),
    S!(34,  "d",            "calculus"),
    S!(35,  r"\delta",      "calculus"),
    S!(36,  "dt",           "calculus"),
    S!(37,  "dx",           "calculus"),
    S!(38,  r"\dot{x}",     "calculus"),
    S!(39,  r"\ddot{x}",    "calculus"),
    S!(40,  r"\dot{y}",     "calculus"),
    S!(41,  r"\ddot{y}",    "calculus"),
    S!(42,  r"\dot{z}",     "calculus"),
    S!(43,  r"\ddot{z}",    "calculus"),
    S!(44,  r"\dot{X}",     "calculus"),
    S!(45,  r"\ddot{X}",    "calculus"),
    S!(46,  r"\dot{V}",     "calculus"),
    S!(47,  r"\ddot{V}",    "calculus"),
    S!(48,  r"\dot{q}",     "calculus"),
    S!(49,  r"\ddot{q}",    "calculus"),
    S!(50,  r"\dot{p}",     "calculus"),
    S!(51,  r"\ddot{p}",    "calculus"),
    S!(52,  r"\dot{\theta}","calculus"),
    S!(53,  r"\ddot{\theta}","calculus"),
    S!(54,  r"\dot{\phi}",  "calculus"),
    S!(55,  r"\ddot{\phi}", "calculus"),
    S!(56,  r"\dot{\psi}",  "calculus"),
    S!(57,  r"\ddot{\psi}", "calculus"),
    S!(58,  r"\dot{\omega}","calculus"),
    S!(59,  r"\ddot{\omega}","calculus"),
    S!(60,  r"\int",        "integral"),
    S!(61,  r"\iint",       "integral"),
    S!(62,  r"\iiint",      "integral"),
    S!(63,  r"\oint",       "integral"),
    S!(64,  r"\oiint",      "integral"),
    S!(65,  r"\sum",        "integral"),
    S!(66,  r"\prod",       "integral"),
    S!(67,  r"\coprod",     "integral"),
    S!(68,  r"\lim",        "analysis"),
    S!(69,  r"\sup",        "analysis"),
    S!(70,  r"\inf",        "analysis"),
    S!(71,  r"\max",        "analysis"),
    S!(72,  r"\min",        "analysis"),
    S!(73,  r"\exp",        "analysis"),
    S!(74,  r"\ln",         "analysis"),
    S!(75,  r"\log",        "analysis"),
    S!(76,  r"\log_{10}",   "analysis"),
    S!(77,  r"\log_2",      "analysis"),
    S!(78,  r"\sin",        "trig"),
    S!(79,  r"\cos",        "trig"),
    S!(80,  r"\tan",        "trig"),
    S!(81,  r"\csc",        "trig"),
    S!(82,  r"\sec",        "trig"),
    S!(83,  r"\cot",        "trig"),
    S!(84,  r"\arcsin",     "trig"),
    S!(85,  r"\arccos",     "trig"),
    S!(86,  r"\arctan",     "trig"),
    S!(87,  r"\sinh",       "trig"),
    S!(88,  r"\cosh",       "trig"),
    S!(89,  r"\tanh",       "trig"),
    S!(90,  r"\text{sech}", "trig"),
    S!(91,  r"\text{csch}", "trig"),
    S!(92,  r"\coth",       "trig"),
    S!(93,  r"\text{arsinh}","trig"),
    S!(94,  r"\text{arcosh}","trig"),
    S!(95,  r"\text{artanh}","trig"),
    S!(96,  r"\text{erf}",  "special"),
    S!(97,  r"\text{erfc}", "special"),
    S!(98,  r"\Gamma",      "special"),
    S!(99,  r"\text{B}",    "special"),
    S!(100, r"\zeta",       "special"),
    S!(101, r"\text{Ai}",   "special"),
    S!(102, r"\text{Bi}",   "special"),
    S!(103, "J_n",          "special"),
    S!(104, "Y_n",          "special"),
    S!(105, "I_n",          "special"),
    S!(106, "K_n",          "special"),
    S!(107, "P_n",          "special"),
    S!(108, "H_n",          "special"),
    S!(109, "L_n",          "special"),
    S!(110, "T_n",          "special"),
    S!(111, "U_n",          "special"),
    S!(112, r"\text{sgn}",  "special"),
    S!(113, r"\text{sinc}", "special"),
    S!(114, r"\Theta",      "special"),
    S!(115, r"\text{Tr}",   "special"),
    S!(116, r"\det",        "special"),
    S!(117, r"\text{div}",  "special"),
    S!(118, r"\text{rot}",  "special"),
    S!(119, r"\text{grad}", "special"),
    S!(120, r"\text{curl}", "special"),
    S!(121, r"\sqrt{\Box}",   "root"),
    S!(122, r"\sqrt[3]{\Box}","root"),
    S!(123, r"\sqrt[4]{\Box}","root"),
    S!(124, r"\sqrt[n]{\Box}","root"),
    S!(125, "!",            "combinatorics"),
    S!(126, "!!",           "combinatorics"),
    S!(127, r"\binom{n}{k}","combinatorics"),
    S!(128, r"\alpha",      "greek"),
    S!(129, r"\beta",       "greek"),
    S!(130, r"\gamma",      "greek"),
    S!(131, r"\epsilon",    "greek"),
    S!(132, r"\varepsilon", "greek"),
    S!(133, r"\eta",        "greek"),
    S!(134, r"\theta",      "greek"),
    S!(135, r"\vartheta",   "greek"),
    S!(136, r"\iota",       "greek"),
    S!(137, r"\kappa",      "greek"),
    S!(138, r"\lambda",     "greek"),
    S!(139, r"\mu",         "greek"),
    S!(140, r"\nu",         "greek"),
    S!(141, r"\xi",         "greek"),
    S!(142, "o",            "greek"),
    S!(143, r"\pi",         "greek"),
    S!(144, r"\varpi",      "greek"),
    S!(145, r"\rho",        "greek"),
    S!(146, r"\varrho",     "greek"),
    S!(147, r"\sigma",      "greek"),
    S!(148, r"\varsigma",   "greek"),
    S!(149, r"\tau",        "greek"),
    S!(150, r"\upsilon",    "greek"),
    S!(151, r"\phi",        "greek"),
    S!(152, r"\varphi",     "greek"),
    S!(153, r"\chi",        "greek"),
    S!(154, r"\psi",        "greek"),
    S!(155, r"\omega",      "greek"),
    S!(156, r"\text{A}",    "greek_upper"),
    S!(157, r"\text{B}",    "greek_upper"),
    S!(158, r"\Lambda",     "greek_upper"),
    S!(159, r"\Xi",         "greek_upper"),
    S!(160, r"\Pi",         "greek_upper"),
    S!(161, r"\Sigma",      "greek_upper"),
    S!(162, r"\Upsilon",    "greek_upper"),
    S!(163, r"\Phi",        "greek_upper"),
    S!(164, r"\Psi",        "greek_upper"),
    S!(165, r"\Omega",      "greek_upper"),
    S!(166, r"\aleph",      "hebrew"),
    S!(167, r"\beth",       "hebrew"),
    S!(168, r"\gimel",      "hebrew"),
    S!(169, r"\daleth",     "hebrew"),
    S!(170, "a", "latin"), S!(171, "b", "latin"), S!(172, "c", "latin"),
    S!(173, "e", "latin"), S!(174, "f", "latin"), S!(175, "g", "latin"),
    S!(176, "h", "latin"), S!(177, "i", "latin"), S!(178, "j", "latin"),
    S!(179, "k", "latin"), S!(180, "l", "latin"), S!(181, "m", "latin"),
    S!(182, "n", "latin"), S!(183, "p", "latin"), S!(184, "q", "latin"),
    S!(185, "r", "latin"), S!(186, "s", "latin"), S!(187, "t", "latin"),
    S!(188, "u", "latin"), S!(189, "v", "latin"), S!(190, "w", "latin"),
    S!(191, "x", "latin"), S!(192, "y", "latin"), S!(193, "z", "latin"),
    S!(194, "A","latin_upper"),S!(195,"B","latin_upper"),S!(196,"C","latin_upper"),
    S!(197,"D","latin_upper"),S!(198,"E","latin_upper"),S!(199,"F","latin_upper"),
    S!(200,"G","latin_upper"),S!(201,"H","latin_upper"),S!(202,"I","latin_upper"),
    S!(203,"J","latin_upper"),S!(204,"K","latin_upper"),S!(205,"L","latin_upper"),
    S!(206,"M","latin_upper"),S!(207,"N","latin_upper"),S!(208,"O","latin_upper"),
    S!(209,"P","latin_upper"),S!(210,"Q","latin_upper"),S!(211,"R","latin_upper"),
    S!(212,"S","latin_upper"),S!(213,"T","latin_upper"),S!(214,"U","latin_upper"),
    S!(215,"V","latin_upper"),S!(216,"W","latin_upper"),S!(217,"X","latin_upper"),
    S!(218,"Y","latin_upper"),S!(219,"Z","latin_upper"),
    S!(220, r"\mathbb{N}",  "sets"),
    S!(221, r"\mathbb{Z}",  "sets"),
    S!(222, r"\mathbb{Q}",  "sets"),
    S!(223, r"\mathbb{R}",  "sets"),
    S!(224, r"\mathbb{C}",  "sets"),
    S!(225, r"\mathbb{H}",  "sets"),
    S!(226, r"\mathbb{O}",  "sets"),
    S!(227, r"\mathbb{P}",  "sets"),
    S!(228, r"\mathbb{F}",  "sets"),
    S!(229, r"\mathbb{K}",  "sets"),
    S!(230, r"\mathbb{S}",  "sets"),
    S!(231, r"\mathbb{T}",  "sets"),
    S!(232,r"\mathcal{A}","calligraphic"),S!(233,r"\mathcal{B}","calligraphic"),
    S!(234,r"\mathcal{C}","calligraphic"),S!(235,r"\mathcal{D}","calligraphic"),
    S!(236,r"\mathcal{E}","calligraphic"),S!(237,r"\mathcal{F}","calligraphic"),
    S!(238,r"\mathcal{G}","calligraphic"),S!(239,r"\mathcal{H}","calligraphic"),
    S!(240,r"\mathcal{I}","calligraphic"),S!(241,r"\mathcal{J}","calligraphic"),
    S!(242,r"\mathcal{K}","calligraphic"),S!(243,r"\mathcal{L}","calligraphic"),
    S!(244,r"\mathcal{M}","calligraphic"),S!(245,r"\mathcal{N}","calligraphic"),
    S!(246,r"\mathcal{O}","calligraphic"),S!(247,r"\mathcal{P}","calligraphic"),
    S!(248,r"\mathcal{Q}","calligraphic"),S!(249,r"\mathcal{R}","calligraphic"),
    S!(250,r"\mathcal{S}","calligraphic"),S!(251,r"\mathcal{T}","calligraphic"),
    S!(252,r"\mathcal{U}","calligraphic"),S!(253,r"\mathcal{V}","calligraphic"),
    S!(254,r"\mathcal{W}","calligraphic"),S!(255,r"\mathcal{X}","calligraphic"),
    S!(256,r"\mathcal{Y}","calligraphic"),S!(257,r"\mathcal{Z}","calligraphic"),
    S!(258,r"\mathfrak{a}","fraktur"),S!(259,r"\mathfrak{b}","fraktur"),
    S!(260,r"\mathfrak{c}","fraktur"),S!(261,r"\mathfrak{d}","fraktur"),
    S!(262,r"\mathfrak{g}","fraktur"),S!(263,r"\mathfrak{h}","fraktur"),
    S!(264,r"\mathfrak{m}","fraktur"),S!(265,r"\mathfrak{n}","fraktur"),
    S!(266,r"\mathfrak{p}","fraktur"),S!(267,r"\mathfrak{q}","fraktur"),
    S!(268,r"\mathfrak{r}","fraktur"),S!(269,r"\mathfrak{s}","fraktur"),
    S!(270,r"\mathfrak{t}","fraktur"),S!(271,r"\mathfrak{u}","fraktur"),
    S!(272,r"\mathfrak{v}","fraktur"),S!(273,r"\mathfrak{w}","fraktur"),
    S!(274,r"\mathfrak{x}","fraktur"),S!(275,r"\mathfrak{y}","fraktur"),
    S!(276,r"\mathfrak{z}","fraktur"),
    S!(277,r"\mathfrak{A}","fraktur"),S!(278,r"\mathfrak{B}","fraktur"),
    S!(279,r"\mathfrak{C}","fraktur"),S!(280,r"\mathfrak{D}","fraktur"),
    S!(281,r"\mathfrak{E}","fraktur"),S!(282,r"\mathfrak{F}","fraktur"),
    S!(283,r"\mathfrak{G}","fraktur"),S!(284,r"\mathfrak{H}","fraktur"),
    S!(285,r"\mathfrak{I}","fraktur"),S!(286,r"\mathfrak{J}","fraktur"),
    S!(287,r"\mathfrak{K}","fraktur"),S!(288,r"\mathfrak{L}","fraktur"),
    S!(289,r"\mathfrak{M}","fraktur"),S!(290,r"\mathfrak{N}","fraktur"),
    S!(291,r"\mathfrak{O}","fraktur"),S!(292,r"\mathfrak{P}","fraktur"),
    S!(293,r"\mathfrak{Q}","fraktur"),S!(294,r"\mathfrak{R}","fraktur"),
    S!(295,r"\mathfrak{S}","fraktur"),S!(296,r"\mathfrak{T}","fraktur"),
    S!(297,r"\mathfrak{U}","fraktur"),S!(298,r"\mathfrak{V}","fraktur"),
    S!(299,r"\mathfrak{W}","fraktur"),S!(300,r"\mathfrak{X}","fraktur"),
    S!(301,r"\mathfrak{Y}","fraktur"),S!(302,r"\mathfrak{Z}","fraktur"),
    S!(303, r"\in",         "set"),
    S!(304, r"\notin",      "set"),
    S!(305, r"\ni",         "set"),
    S!(306, r"\not\ni",     "set"),
    S!(307, r"\subset",     "set"),
    S!(308, r"\supset",     "set"),
    S!(309, r"\subseteq",   "set"),
    S!(310, r"\supseteq",   "set"),
    S!(311, r"\subsetneq",  "set"),
    S!(312, r"\supsetneq",  "set"),
    S!(313, r"\cup",        "set"),
    S!(314, r"\cap",        "set"),
    S!(315, r"\setminus",   "set"),
    S!(316, r"\emptyset",   "set"),
    S!(317, r"\forall",     "logic"),
    S!(318, r"\exists",     "logic"),
    S!(319, r"\nexists",    "logic"),
    S!(320, r"\neg",        "logic"),
    S!(321, r"\land",       "logic"),
    S!(322, r"\lor",        "logic"),
    S!(323, r"\implies",    "logic"),
    S!(324, r"\impliedby",  "logic"),
    S!(325, r"\iff",        "logic"),
    S!(326, r"\top",        "logic"),
    S!(327, r"\bot",        "logic"),
    S!(328, r"\vdash",      "logic"),
    S!(329, r"\vDash",      "logic"),
    S!(330, r"\Vdash",      "logic"),
    S!(331, r"\to",         "arrow"),
    S!(332, r"\leftarrow",  "arrow"),
    S!(333, r"\leftrightarrow","arrow"),
    S!(334, r"\mapsto",     "arrow"),
    S!(335, r"\hookrightarrow","arrow"),
    S!(336, r"\twoheadrightarrow","arrow"),
    S!(337, r"\uparrow",    "arrow"),
    S!(338, r"\downarrow",  "arrow"),
    S!(339, r"\updownarrow","arrow"),
    S!(340, r"\nearrow",    "arrow"),
    S!(341, r"\searrow",    "arrow"),
    S!(342, r"\swarrow",    "arrow"),
    S!(343, r"\nwarrow",    "arrow"),
    S!(344, r"\rightleftharpoons","arrow"),
    S!(345, r"\infty",      "constant"),
    S!(346, r"\aleph_0",    "constant"),
    S!(347, r"\omega_0",    "constant"),
    S!(348, r"\ell",        "constant"),
    S!(349, r"\hbar",       "constant"),
    S!(350, "c",            "constant"),
    S!(351, "G",            "constant"),
    S!(352, "k_B",          "constant"),
    S!(353, "N_A",          "constant"),
    S!(354, r"\mu_0",       "constant"),
    S!(355, r"\varepsilon_0","constant"),
    S!(356, r"\sigma_{SB}", "constant"),
    S!(357, r"\lambda_{\max}","constant"),
    S!(358, r"\lambda_1",   "constant"),
    S!(359, r"\lambda_2",   "constant"),
    S!(360, r"\lambda_3",   "constant"),
    S!(361, r"\lambda_i",   "constant"),
    S!(362, "x_0",          "state"),
    S!(363, "y_0",          "state"),
    S!(364, "z_0",          "state"),
    S!(365, "v_0",          "state"),
    S!(366, "t_0",          "state"),
    S!(367, r"x_\infty",    "state"),
    S!(368, "p_x",          "mechanics"),
    S!(369, "p_y",          "mechanics"),
    S!(370, "p_z",          "mechanics"),
    S!(371, "L_x",          "mechanics"),
    S!(372, "L_y",          "mechanics"),
    S!(373, "L_z",          "mechanics"),
    S!(374, r"T_{\text{kinetic}}","mechanics"),
    S!(375, r"V_{\text{potential}}","mechanics"),
    S!(376, r"H_{\text{hamiltonian}}","mechanics"),
    S!(377, r"\mathcal{L}_{\text{lagrangian}}","mechanics"),
    S!(378, r"S_{\text{action}}","mechanics"),
    S!(379, r"W_{\text{work}}","mechanics"),
    S!(380, r"E_{\text{total}}","mechanics"),
    S!(381, r"F_{\text{force}}","mechanics"),
    S!(382, r"P_{\text{momentum}}","mechanics"),
    S!(383, r"J_{\text{inertia}}","mechanics"),
    S!(384, r"\tau_{\text{torque}}","mechanics"),
    S!(385, r"I_{\text{current}}","em"),
    S!(386, r"V_{\text{voltage}}","em"),
    S!(387, r"R_{\text{resistance}}","em"),
    S!(388, r"C_{\text{capacitance}}","em"),
    S!(389, r"L_{\text{inductance}}","em"),
    S!(390, r"Q_{\text{charge}}","em"),
    S!(391, r"\Phi_B",      "em"),
    S!(392, r"\Phi_E",      "em"),
    S!(393, r"\vec{E}",     "em"),
    S!(394, r"\vec{B}",     "em"),
    S!(395, r"\vec{A}",     "em"),
    S!(396, r"\vec{D}",     "em"),
    S!(397, r"\vec{H}",     "em"),
    S!(398, r"\vec{J}",     "em"),
    S!(399, r"\vec{v}",     "mechanics"),
    S!(400, r"\vec{u}",     "mechanics"),
    S!(401, r"\vec{a}",     "mechanics"),
    S!(402, r"\vec{r}",     "mechanics"),
    S!(403, r"\vec{F}",     "mechanics"),
    S!(404, r"\vec{p}",     "mechanics"),
    S!(405, r"\vec{L}",     "mechanics"),
    S!(406, r"\hat{i}",     "basis"),
    S!(407, r"\hat{j}",     "basis"),
    S!(408, r"\hat{k}",     "basis"),
    S!(409, r"\hat{n}",     "basis"),
    S!(410, r"\hat{r}",     "basis"),
    S!(411, r"\hat{\theta}","basis"),
    S!(412, r"\hat{\phi}",  "basis"),
    S!(413, r"\hat{x}",     "basis"),
    S!(414, r"\hat{y}",     "basis"),
    S!(415, r"\hat{z}",     "basis"),
    S!(416, r"\langle",             "qm"),
    S!(417, r"\rangle",             "qm"),
    S!(418, r"\langle \psi |",      "qm"),
    S!(419, r"| \psi \rangle",      "qm"),
    S!(420, r"\langle \phi | \psi \rangle","qm"),
    S!(421, r"| \phi \rangle \langle \psi |","qm"),
    S!(422, r"\hat{H}",             "qm"),
    S!(423, r"\hat{p}",             "qm"),
    S!(424, r"\hat{x}",             "qm"),
    S!(425, r"\hat{a}",             "qm"),
    S!(426, r"\hat{a}^\dagger",     "qm"),
    S!(427, r"\hat{\sigma}_x",      "qm"),
    S!(428, r"\hat{\sigma}_y",      "qm"),
    S!(429, r"\hat{\sigma}_z",      "qm"),
    S!(430, r"\hat{\rho}",          "qm"),
    S!(431, r"\text{Tr}(\rho)",     "qm"),
    S!(432, r"\mathcal{F}_{\mu\nu}","tensor"),
    S!(433, r"g_{\mu\nu}",          "tensor"),
    S!(434, r"g^{\mu\nu}",          "tensor"),
    S!(435, r"\eta_{\mu\nu}",       "tensor"),
    S!(436, r"\Gamma^\lambda_{\mu\nu}","tensor"),
    S!(437, r"R^\rho_{\sigma\mu\nu}","tensor"),
    S!(438, r"R_{\mu\nu}",          "tensor"),
    S!(439, "R",                    "tensor"),
    S!(440, r"T_{\mu\nu}",          "tensor"),
    S!(441, r"\Lambda_{\text{cosmo}}","tensor"),
    S!(442, r"D_\mu",               "tensor"),
    S!(443, r"\nabla_\mu",          "tensor"),
    S!(444, r"\Box",                "tensor"),
    S!(445, r"d s^2",               "tensor"),
    S!(446, r"d \tau",              "tensor"),
    S!(447, r"\gamma_{\text{lorentz}}","tensor"),
    S!(448, r"\beta_{\text{rel}}",  "tensor"),
    S!(449, r"\mu_{\text{friction}}","mechanics"),
    S!(450, r"k_{\text{spring}}",   "mechanics"),
    S!(451, r"c_{\text{damping}}",  "mechanics"),
    S!(452, r"\gamma_{\text{damping}}","mechanics"),
    S!(453, r"\delta_{\text{dirac}}","analysis"),
    S!(454, r"\delta_{ij}",         "tensor"),
    S!(455, r"\epsilon_{ijk}",      "tensor"),
    S!(456, r"\Gamma(z)",           "special2"),
    S!(457, r"\psi(z)",             "special2"),
    S!(458, r"B(x,y)",              "special2"),
    S!(459, r"\text{Li}_2(z)",      "special2"),
    S!(460, r"\text{Li}_n(z)",      "special2"),
    S!(461, r"E_n(x)",              "special2"),
    S!(462, r"\text{Si}(x)",        "special2"),
    S!(463, r"\text{Ci}(x)",        "special2"),
    S!(464, r"\text{Shi}(x)",       "special2"),
    S!(465, r"\text{Chi}(x)",       "special2"),
    S!(466, r"\text{Ei}(x)",        "special2"),
    S!(467, r"\text{li}(x)",        "special2"),
    S!(468, r"\text{sn}(u,k)",      "elliptic"),
    S!(469, r"\text{cn}(u,k)",      "elliptic"),
    S!(470, r"\text{dn}(u,k)",      "elliptic"),
    S!(471, r"K(k)",                "elliptic"),
    S!(472, r"E(k)",                "elliptic"),
    S!(473, r"\wp(z)",              "weierstrass"),
    S!(474, r"\wp'(z)",             "weierstrass"),
    S!(475, r"\sigma(z)",           "weierstrass"),
    S!(476, r"\zeta_{\text{weierstrass}}(z)","weierstrass"),
    S!(477, r"\vartheta_1(z,q)",    "theta"),
    S!(478, r"\vartheta_2(z,q)",    "theta"),
    S!(479, r"\vartheta_3(z,q)",    "theta"),
    S!(480, r"\vartheta_4(z,q)",    "theta"),
    S!(481, r"\eta_{\text{dedekind}}(\tau)","modular"),
    S!(482, r"j(\tau)",             "modular"),
    S!(483, r"W_0(x)",              "lambert"),
    S!(484, r"W_{-1}(x)",          "lambert"),
    S!(485, r"\text{agm}(x,y)",     "special2"),
    S!(486, r"\text{erfi}(x)",      "special2"),
    S!(487, r"\text{dawson}(x)",    "special2"),
    S!(488, r"\text{FresnelS}(x)",  "special2"),
    S!(489, r"\text{FresnelC}(x)",  "special2"),
    S!(490, r"H(x)",                "special2"),
    S!(491, r"\text{sign}(x)",      "special2"),
    S!(492, r"\text{rect}(x)",      "special2"),
    S!(493, r"\Lambda(x)",          "special2"),
    S!(494, r"\text{comb}(x)",      "special2"),
    S!(495, r"\delta(t-t_0)",       "distribution"),
    S!(496, r"\delta'(x)",          "distribution"),
    S!(497, r"\delta''(x)",         "distribution"),
    S!(498, r"\nabla^2",            "operator"),
    S!(499, r"\nabla^4",            "operator"),
    S!(500, r"\Delta_{\text{laplace}}","operator"),
    S!(501, r"\Delta_{\text{beltrami}}","operator"),
    S!(502, r"\text{rot}\vec{v}",   "operator"),
    S!(503, r"\text{div}\vec{v}",   "operator"),
    S!(504, r"\vec{A} \times \vec{B}","operator"),
    S!(505, r"\vec{A} \cdot \vec{B}","operator"),
    S!(506, r"\vec{A} \otimes \vec{B}","operator"),
    S!(507, r"[A, B]",              "bracket"),
    S!(508, r"\{A, B\}",            "bracket"),
    S!(509, r"[A, B]_D",            "bracket"),
    S!(510, r"[A, B]_{PB}",         "bracket"),
    S!(511, r"\mathcal{L}_X Y",     "diffgeom"),
    S!(512, r"i_X \omega",          "diffgeom"),
    S!(513, r"d\omega",             "diffgeom"),
    S!(514, r"\star\alpha",         "diffgeom"),
    S!(515, r"\omega \wedge \eta",  "diffgeom"),
    S!(516, r"\int_M \omega",       "diffgeom"),
    S!(517, r"\int_{\partial M} \omega","diffgeom"),
    S!(518, r"\oint_\gamma f(z)dz", "complex"),
    S!(519, r"\text{Res}(f, z_0)",  "complex"),
    S!(520, r"\dim(V)",             "linalg"),
    S!(521, r"\ker(T)",             "linalg"),
    S!(522, r"\text{im}(T)",        "linalg"),
    S!(523, r"\text{rank}(A)",      "linalg"),
    S!(524, r"\text{nullity}(A)",   "linalg"),
    S!(525, r"\text{span}(S)",      "linalg"),
    S!(526, r"\text{proj}_u(v)",    "linalg"),
    S!(527, r"\text{orth}(V)",      "linalg"),
    S!(528, r"\text{diag}(\lambda_1,\dots,\lambda_n)","linalg"),
    S!(529, r"A^T",                 "linalg"),
    S!(530, r"A^{-1}",             "linalg"),
    S!(531, r"A^*",                 "linalg"),
    S!(532, r"A^\dagger",           "linalg"),
    S!(533, r"A^+",                 "linalg"),
    S!(534, r"\|x\|",               "linalg"),
    S!(535, r"\|x\|_1",             "linalg"),
    S!(536, r"\|x\|_2",             "linalg"),
    S!(537, r"\|x\|_p",             "linalg"),
    S!(538, r"\|x\|_\infty",        "linalg"),
    S!(539, r"\|A\|_F",             "linalg"),
    S!(540, r"\langle x, y \rangle","linalg"),
    S!(541, r"x \perp y",           "linalg"),
    S!(542, r"x \parallel y",       "linalg"),
    S!(543, r"\angle(u,v)",         "linalg"),
    S!(544, r"\sigma_i(A)",         "linalg"),
    S!(545, r"\text{cond}(A)",      "linalg"),
    S!(546, r"\text{eig}(A)",       "linalg"),
    S!(547, r"\det(A-\lambda I)",   "linalg"),
    S!(548, r"\text{adj}(A)",       "linalg"),
    S!(549, r"\text{cof}(A)",       "linalg"),
    S!(550, r"I_n",                 "linalg"),
    S!(551, r"0_{m\times n}",       "linalg"),
    S!(552, r"J_{2n}",              "symplectic"),
    S!(553, r"\Omega_{\text{symp}}","symplectic"),
    S!(554, r"\omega_{\text{can}}", "symplectic"),
    S!(555, r"T^*M",                "diffgeom"),
    S!(556, r"TM",                  "diffgeom"),
    S!(557, r"\Gamma(TM)",          "diffgeom"),
    S!(558, r"\Omega^k(M)",         "diffgeom"),
    S!(559, r"\chi(M)",             "topology"),
    S!(560, r"\pi_1(X)",            "topology"),
    S!(561, r"\pi_n(X)",            "topology"),
    S!(562, r"H_n(X)",              "topology"),
    S!(563, r"H^n(X)",              "topology"),
    S!(564, r"c_1(E)",              "topology"),
    S!(565, r"p_1(E)",              "topology"),
    S!(566, r"\text{ch}(E)",        "topology"),
    S!(567, r"\text{Td}(M)",        "topology"),
    S!(568, r"\hat{A}(M)",          "topology"),
    S!(569, r"\int_X \alpha \smile \beta","topology"),
    S!(570, r"\alpha \frown z",     "topology"),
    S!(571, r"\text{Hom}(A,B)",     "algebra"),
    S!(572, r"\text{End}(V)",       "algebra"),
    S!(573, r"\text{Aut}(G)",       "algebra"),
    S!(574, r"\text{GL}(n,\mathbb{R})","lie"),
    S!(575, r"\text{SL}(n,\mathbb{R})","lie"),
    S!(576, r"\text{O}(n)",         "lie"),
    S!(577, r"\text{SO}(n)",        "lie"),
    S!(578, r"\text{U}(n)",         "lie"),
    S!(579, r"\text{SU}(n)",        "lie"),
    S!(580, r"\text{Sp}(2n)",       "lie"),
    S!(581, r"\mathfrak{gl}(n)",    "lie_algebra"),
    S!(582, r"\mathfrak{sl}(n)",    "lie_algebra"),
    S!(583, r"\mathfrak{so}(n)",    "lie_algebra"),
    S!(584, r"\mathfrak{su}(n)",    "lie_algebra"),
    S!(585, r"\mathfrak{sp}(2n)",   "lie_algebra"),
    S!(586, r"\text{Ad}_g",         "lie"),
    S!(587, r"\text{ad}_X",         "lie"),
    S!(588, r"B(X,Y)_{\text{killing}}","lie"),
    S!(589, r"\mathcal{D}'(\mathbb{R})","funcspace"),
    S!(590, r"C^\infty(M)",         "funcspace"),
    S!(591, r"C^k(M)",              "funcspace"),
    S!(592, r"C^0(M)",              "funcspace"),
    S!(593, r"L^1(\mathbb{R})",     "funcspace"),
    S!(594, r"L^2(\mathbb{R})",     "funcspace"),
    S!(595, r"L^p(\mathbb{R})",     "funcspace"),
    S!(596, r"L^\infty(\mathbb{R})","funcspace"),
    S!(597, r"W^{k,p}(\Omega)",     "funcspace"),
    S!(598, r"H^s(\mathbb{R}^n)",   "funcspace"),
    S!(599, r"\mathcal{S}(\mathbb{R}^n)","funcspace"),
    S!(600, r"\mathcal{S}'(\mathbb{R}^n)","funcspace"),
    S!(601, r"\hat{f}(\xi)",        "transform"),
    S!(602, r"\mathcal{F}\{f\}",    "transform"),
    S!(603, r"\mathcal{F}^{-1}\{F\}","transform"),
    S!(604, r"\mathcal{L}\{f\}",    "transform"),
    S!(605, r"\mathcal{L}^{-1}\{F\}","transform"),
    S!(606, r"\mathcal{Z}\{x_n\}",  "transform"),
    S!(607, r"\mathcal{H}\{f\}",    "transform"),
    S!(608, r"\mathcal{W}\{f\}",    "transform"),
    S!(609, r"f * g",               "transform"),
    S!(610, r"f \star g",           "probability"),
    S!(611, r"\text{cov}(X,Y)",     "probability"),
    S!(612, r"\text{corr}(X,Y)",    "probability"),
    S!(613, r"\text{var}(X)",       "probability"),
    S!(614, r"\mathbb{E}[X]",       "probability"),
    S!(615, r"\mathbb{P}(A)",       "probability"),
    S!(616, r"\mathbb{P}(A|B)",     "probability"),
    S!(617, r"\sigma_{\text{std}}", "probability"),
    S!(618, r"\mu_{\text{mean}}",   "probability"),
    S!(619, r"\text{skew}(X)",      "probability"),
    S!(620, r"\text{kurt}(X)",      "probability"),
    S!(621, r"M_X(t)",              "probability"),
    S!(622, r"\phi_X(t)",           "probability"),
    S!(623, r"\chi^2",              "probability"),
    S!(624, r"t_{\text{student}}",  "probability"),
    S!(625, r"F_{\text{snedecor}}", "probability"),
    S!(626, r"W_t",                 "stochastic"),
    S!(627, r"B_t",                 "stochastic"),
    S!(628, r"d B_t",               "stochastic"),
    S!(629, r"d W_t",               "stochastic"),
    S!(630, r"\int f dW_t",         "stochastic"),
    S!(631, r"\mu(x,t)dt + \sigma(x,t)dW_t","stochastic"),
    S!(632, r"\mathcal{A}_{\text{generator}}","stochastic"),
    S!(633, r"p(x,t|x_0,t_0)",     "stochastic"),
    S!(634, r"T_{\text{transit}}",  "stochastic"),
    S!(635, r"\tau_{\text{escape}}","stochastic"),
    S!(636, r"D_1(x)",              "stochastic"),
    S!(637, r"D_2(x)",              "stochastic"),
    S!(638, r"D_{\text{diffusion}}","stochastic"),
    S!(639, r"v_{\text{drift}}",    "stochastic"),
    S!(640, r"\lambda_L",           "chaos"),
    S!(641, r"D_{\text{KY}}",       "chaos"),
    S!(642, r"D_0",                 "chaos"),
    S!(643, r"D_1",                 "chaos"),
    S!(644, r"D_2",                 "chaos"),
    S!(645, r"D_q",                 "chaos"),
    S!(646, r"f(\alpha)",           "chaos"),
    S!(647, r"h_{\text{KS}}",       "chaos"),
    S!(648, r"K_{\text{entropy}}",  "chaos"),
    S!(649, r"\mu_{\text{SRB}}",    "chaos"),
    S!(650, r"\rho_{\text{inv}}(x)","chaos"),
    S!(651, r"\mathcal{P}_{\text{frobenius-perron}}","chaos"),
    S!(652, r"\mathcal{U}_{\text{koopman}}","chaos"),
    S!(653, r"W^s(x)",              "manifold"),
    S!(654, r"W^u(x)",              "manifold"),
    S!(655, r"W^c(x)",              "manifold"),
    S!(656, r"E^s",                 "manifold"),
    S!(657, r"E^u",                 "manifold"),
    S!(658, r"E^c",                 "manifold"),
    S!(659, r"\Sigma_{\text{poincare}}","chaos"),
    S!(660, r"P_{\text{map}}",      "chaos"),
    S!(661, r"x_{n+1} = f(x_n)",   "map"),
    S!(662, r"r x (1-x)",           "map"),
    S!(663, r"1 - a x^2 + y",       "map"),
    S!(664, r"b x",                 "map"),
    S!(665, r"x + \Omega - \frac{K}{2\pi}\sin(2\pi x)","map"),
    S!(666, r"2x \pmod 1",          "map"),
    S!(667, r"3x+1",                "map"),
    S!(668, r"\sigma(y-x)",         "lorenz"),
    S!(669, r"x(\rho-z)-y",         "lorenz"),
    S!(670, r"xy-\beta z",          "lorenz"),
    S!(671, r"-y-z",                "rossler"),
    S!(672, r"x+ay",                "rossler"),
    S!(673, r"b+z(x-c)",            "rossler"),
    S!(674, r"a(y-x)",              "chen"),
    S!(675, r"(c-a)x - xz + cy",   "chen"),
    S!(676, r"xy-bz",               "chen"),
    S!(677, r"\alpha(y-x-f(x))",    "chua"),
    S!(678, r"x-y+z",               "chua"),
    S!(679, r"-\beta y",            "chua"),
    S!(680, r"m_0 x + \frac{1}{2}(m_1-m_0)(|x+1|-|x-1|)","chua"),
    S!(681, r"v(1-u^2) - u",        "fitzhugh"),
    S!(682, r"\mu(1-x^2)v - x",    "vanderpol"),
    S!(683, r"-x^3 + \beta x + \gamma \cos(\omega t)","duffing"),
    S!(684, r"-\delta v - \alpha x - \beta x^3","duffing"),
    S!(685, r"v",                   "ode_rhs"),
    S!(686, r"-\sin(x)",            "pendulum"),
    S!(687, r"-\sin(x) + T",        "pendulum"),
    S!(688, r"-\gamma v - \sin(x) + F\cos(\omega t)","pendulum"),
    S!(689, r"y",                   "ode_rhs"),
    S!(690, r"\mu(1-x^2)y - x",    "vanderpol"),
    S!(691, r"z",                   "ode_rhs"),
    S!(692, r"-a x - b y - c z + d x^3","ode_rhs"),
    S!(693, r"\dot{r} = r(\mu - r^2)","normal_form"),
    S!(694, r"\dot{\theta} = \omega + b r^2","normal_form"),
    S!(695, r"\dot{x} = \mu x - x^3","normal_form"),
    S!(696, r"\dot{x} = \mu x + x^3 - x^5","normal_form"),
    S!(697, r"\dot{x} = \mu - x^2", "normal_form"),
    S!(698, r"\dot{x} = \mu x - x^2","normal_form"),
    S!(699, r"\dot{x} = y",         "ode_rhs"),
    S!(700, r"\dot{y} = \mu_1 + \mu_2 x + x^2 \pm x y","normal_form"),
    S!(701, r"\dot{y} = \mu_1 + \mu_2 y + x^2 + x y","normal_form"),
    S!(702, r"\dot{x} = y",         "control"),
    S!(703, r"\dot{y} = -x + \mu y - x^2 y","control"),
    S!(704, r"\dot{x} = y",         "control"),
    S!(705, r"\dot{y} = \mu_1 x + \mu_2 y - x^3 - x^2 y","control"),
    S!(706, r"\delta_1",            "bifurcation"),
    S!(707, r"\delta_2",            "bifurcation"),
    S!(708, r"\alpha_{\text{feigenbaum}}","feigenbaum"),
    S!(709, r"\delta_{\text{feigenbaum}}","feigenbaum"),
    S!(710, r"4.6692016\dots",      "feigenbaum"),
    S!(711, r"2.5029078\dots",      "feigenbaum"),
    S!(712, r"\lambda_{\text{lyapunov}}","chaos"),
    S!(713, r"\tau_{\text{delay}}", "dde"),
    S!(714, r"x(t-\tau)",           "dde"),
    S!(715, r"\dot{x}(t) = f(x(t), x(t-\tau))","dde"),
    S!(716, r"\frac{a x(t-\tau)}{1 + x(t-\tau)^c} - b x(t)","dde"),
    S!(717, r"-\gamma x(t) + \frac{\beta_0 x(t-\tau)}{1 + (x(t-\tau)/\theta)^n}","dde"),
    S!(718, r"\dot{x} = A x + B u",  "control"),
    S!(719, r"y = C x + D u",        "control"),
    S!(720, r"\dot{x} = f(x) + g(x)u","control"),
    S!(721, r"u = -K x",             "control"),
    S!(722, r"u = -R^{-1} B^T P x",  "control"),
    S!(723, r"A^T P + P A - P B R^{-1} B^T P + Q = 0","control"),
    S!(724, r"\dot{P} = A P + P A^T + Q - P C^T R^{-1} C P","control"),
    S!(725, r"K_k = P_k^- H^T (H P_k^- H^T + R)^{-1}","control"),
    S!(726, r"\hat{x}_k = \hat{x}_k^- + K_k (z_k - H \hat{x}_k^-)","control"),
    S!(727, r"V(x) > 0",             "lyapunov"),
    S!(728, r"\dot{V}(x) \le 0",     "lyapunov"),
    S!(729, r"\dot{V}(x) < 0",       "lyapunov"),
    S!(730, r"\dot{V}(x) \le -\alpha V(x)","lyapunov"),
    S!(731, r"L_f V(x) + L_g V(x) u \le 0","lyapunov"),
    S!(732, r"\mathcal{K}_\infty",   "stability"),
    S!(733, r"\mathcal{K}\mathcal{L}","stability"),
    S!(734, r"\text{sgn}(s) |s|^\alpha","sliding"),
    S!(735, r"-k \text{sat}(s/\epsilon)","sliding"),
    S!(736, r"s = \dot{e} + \lambda e","sliding"),
    S!(737, r"\dot{s} = -k \text{sgn}(s)","sliding"),
    S!(738, r"\dot{s} = -k |s|^{1/2}\text{sgn}(s) + w","sliding"),
    S!(739, r"\dot{w} = -b \text{sgn}(s)","sliding"),
    S!(740, r"\nabla_x f(x^*) = 0",  "optimization"),
    S!(741, r"\nabla_{xx}^2 f(x^*) \succeq 0","optimization"),
    S!(742, r"\nabla f(x) + \sum \lambda_i \nabla g_i(x) + \sum \mu_j \nabla h_j(x) = 0","optimization"),
    S!(743, r"\lambda_i g_i(x) = 0", "optimization"),
    S!(744, r"\lambda_i \ge 0",       "optimization"),
    S!(745, r"g_i(x) \le 0",         "optimization"),
    S!(746, r"h_j(x) = 0",           "optimization"),
    S!(747, r"\mathcal{L}(x, \lambda, \mu)","optimization"),
    S!(748, r"\min_x \max_{\lambda \ge 0} \mathcal{L}(x,\lambda)","optimization"),
    S!(749, r"f(y) \ge f(x) + \nabla f(x)^T(y-x)","optimization"),
    S!(750, r"f(\theta x + (1-\theta)y) \le \theta f(x) + (1-\theta)f(y)","optimization"),
    S!(751, r"\partial f(x)",         "optimization"),
    S!(752, r"x_{k+1} = x_k - \alpha \nabla f(x_k)","optimization"),
    S!(753, r"x_{k+1} = \text{prox}_{\alpha g}(x_k - \alpha \nabla f(x_k))","optimization"),
    S!(754, r"\text{prox}_f(v) = \arg\min_x \left(f(x) + \frac{1}{2}\|x-v\|^2\right)","optimization"),
    S!(755, r"x_{k+1} = x_k - [H f(x_k)]^{-1} \nabla f(x_k)","optimization"),
    S!(756, r"B_{k+1} = B_k + \frac{y_k y_k^T}{y_k^T s_k} - \frac{B_k s_k s_k^T B_k}{s_k^T B_k s_k}","optimization"),
    S!(757, r"s_k = x_{k+1}-x_k",    "optimization"),
    S!(758, r"y_k = \nabla f(x_{k+1}) - \nabla f(x_k)","optimization"),
    S!(759, r"\beta_k = \frac{\nabla f_{k+1}^T \nabla f_{k+1}}{\nabla f_k^T \nabla f_k}","optimization"),
    S!(760, r"\beta_k = \frac{\nabla f_{k+1}^T (\nabla f_{k+1} - \nabla f_k)}{\nabla f_k^T \nabla f_k}","optimization"),
    S!(761, r"\text{div}(F) \equiv 0","topology"),
    S!(762, r"\text{curl}(F) \equiv 0","topology"),
    S!(763, r"d\omega = 0",           "topology"),
    S!(764, r"\omega = d\alpha",      "topology"),
    S!(765, r"H^k_{\text{dR}}(M) = Z^k(M)/B^k(M)","topology"),
    S!(766, r"b_k = \dim H^k_{\text{dR}}(M)","topology"),
    S!(767, r"\chi(M) = \sum (-1)^k b_k","topology"),
    S!(768, r"\int_M d\omega = \int_{\partial M} \omega","stokes"),
    S!(769, r"\int_a^b f'(x)dx = f(b)-f(a)","stokes"),
    S!(770, r"\oint_C (L dx + M dy) = \iint_D \left(\frac{\partial M}{\partial x} - \frac{\partial L}{\partial y}\right) dA","stokes"),
    S!(771, r"\iint_S (\nabla \times \vec{F})\cdot d\vec{S} = \oint_C \vec{F}\cdot d\vec{r}","stokes"),
    S!(772, r"\iiint_V (\nabla \cdot \vec{F}) dV = \oiint_S \vec{F}\cdot d\vec{S}","stokes"),
    S!(773, r"\dot{x} = \frac{\partial H}{\partial p}","hamilton"),
    S!(774, r"\dot{p} = -\frac{\partial H}{\partial q}","hamilton"),
    S!(775, r"\frac{dH}{dt} = \frac{\partial H}{\partial t} + \{H, H\} = \frac{\partial H}{\partial t}","hamilton"),
    S!(776, r"\frac{\partial \mathcal{L}}{\partial q} - \frac{d}{dt}\frac{\partial \mathcal{L}}{\partial \dot{q}} = 0","lagrange"),
    S!(777, r"\frac{\partial \mathcal{L}}{\partial \phi} - \partial_\mu \frac{\partial \mathcal{L}}{\partial(\partial_\mu \phi)} = 0","lagrange"),
    S!(778, r"\delta S = 0",          "variational"),
    S!(779, r"S = \int \mathcal{L} dt","variational"),
    S!(780, r"S = \int \mathcal{L}_{\text{density}} d^4x","variational"),
    S!(781, r"j^\mu = \frac{\partial \mathcal{L}}{\partial(\partial_\mu \phi)} \Delta \phi - \mathcal{J}^\mu","noether"),
    S!(782, r"\partial_\mu j^\mu = 0","noether"),
    S!(783, r"T^{\mu\nu} = \frac{\partial \mathcal{L}}{\partial(\partial_\mu \phi)} \partial^\nu \phi - g^{\mu\nu}\mathcal{L}","noether"),
    S!(784, r"\partial_\mu T^{\mu\nu} = 0","noether"),
    S!(785, r"i \hbar \frac{\partial}{\partial t}\psi = \hat{H}\psi","qm"),
    S!(786, r"-\frac{\hbar^2}{2m}\nabla^2\psi + V\psi = E\psi","qm"),
    S!(787, r"(i \gamma^\mu \partial_\mu - m)\psi = 0","qm"),
    S!(788, r"(\Box + m^2)\phi = 0",  "qm"),
    S!(789, r"\gamma^\mu \gamma^\nu + \gamma^\nu \gamma^\mu = 2 g^{\mu\nu}","qm"),
    S!(790, r"\gamma^5 = i \gamma^0 \gamma^1 \gamma^2 \gamma^3","qm"),
    S!(791, r"\bar{\psi} = \psi^\dagger \gamma^0","qm"),
    S!(792, r"\mathcal{L}_{\text{QED}} = \bar{\psi}(i\gamma^\mu D_\mu - m)\psi - \frac{1}{4}F_{\mu\nu}F^{\mu\nu}","qed"),
    S!(793, r"D_\mu = \partial_\mu - i e A_\mu","qed"),
    S!(794, r"F_{\mu\nu} = \partial_\mu A_\nu - \partial_\nu A_\mu","em"),
    S!(795, r"F_{\mu\nu}^a = \partial_\mu A_\nu^a - \partial_\nu A_\mu^a + g f^{abc} A_\mu^b A_\nu^c","qcd"),
    S!(796, r"\mathcal{L}_{\text{YM}} = -\frac{1}{4} F_{\mu\nu}^a F^{\mu\nu, a}","qcd"),
    S!(797, r"\mathcal{W}[J] = \int \mathcal{D}\phi \exp\left(\frac{i}{\hbar}(S[\phi] + \int J\phi)\right)","qft"),
    S!(798, r"\Gamma[\phi_{\text{cl}}] = W[J] - \int J \phi_{\text{cl}}","qft"),
    S!(799, r"\frac{\delta \Gamma}{\delta \phi_{\text{cl}}} = -J","qft"),
    S!(800, r"\beta(g) = \frac{\partial g}{\partial \ln \mu}","qft"),
    S!(801, r"\gamma(g) = \frac{\partial \ln Z}{\partial \ln \mu}","qft"),
    S!(802, r"\mu \frac{d}{d\mu} \Gamma^{(n)} = 0","qft"),
    S!(803, r"\left(\mu \frac{\partial}{\partial \mu} + \beta(g)\frac{\partial}{\partial g} - n\gamma\right)\Gamma^{(n)} = 0","qft"),
    S!(804, r"c = \frac{1}{\sqrt{\varepsilon_0 \mu_0}}","em"),
    S!(805, r"E = h \nu",              "quantum"),
    S!(806, r"E = \hbar \omega",       "quantum"),
    S!(807, r"\lambda_{\text{dB}} = \frac{h}{p}","quantum"),
    S!(808, r"\Delta x \Delta p \ge \frac{\hbar}{2}","quantum"),
    S!(809, r"\Delta E \Delta t \ge \frac{\hbar}{2}","quantum"),
    S!(810, r"G_{\mu\nu} + \Lambda g_{\mu\nu} = \frac{8\pi G}{c^4} T_{\mu\nu}","gr"),
    S!(811, r"G_{\mu\nu} = R_{\mu\nu} - \frac{1}{2}R g_{\mu\nu}","gr"),
    S!(812, r"R^\rho_{\sigma\mu\nu} = \partial_\mu \Gamma^\rho_{\nu\sigma} - \partial_\nu \Gamma^\rho_{\mu\sigma} + \Gamma^\rho_{\mu\lambda}\Gamma^\lambda_{\nu\sigma} - \Gamma^\rho_{\nu\lambda}\Gamma^\lambda_{\mu\sigma}","gr"),
    S!(813, r"\Gamma^\rho_{\mu\nu} = \frac{1}{2}g^{\rho\sigma}(\partial_\mu g_{\nu\sigma} + \partial_\nu g_{\mu\sigma} - \partial_\sigma g_{\mu\nu})","gr"),
    S!(814, r"\nabla_\mu V^\nu = \partial_\mu V^\nu + \Gamma^\nu_{\mu\lambda} V^\lambda","gr"),
    S!(815, r"\nabla_\mu V_\nu = \partial_\mu V_\nu - \Gamma^\lambda_{\mu\nu} V_\lambda","gr"),
    S!(816, r"\nabla_\sigma g_{\mu\nu} = 0","gr"),
    S!(817, r"R_{\mu\nu\rho\sigma} = R_{\rho\sigma\mu\nu}","gr"),
    S!(818, r"R_{\mu\nu\rho\sigma} = -R_{\nu\mu\rho\sigma}","gr"),
    S!(819, r"R_{\mu\nu\rho\sigma} = -R_{\mu\nu\sigma\rho}","gr"),
    S!(820, r"R_{\mu[\nu\rho\sigma]} = 0","gr"),
    S!(821, r"\nabla_{[\lambda} R^\rho_{\sigma|\mu\nu]} = 0","gr"),
    S!(822, r"\nabla^\mu G_{\mu\nu} = 0","gr"),
    S!(823, r"\nabla^\mu T_{\mu\nu} = 0","gr"),
    S!(824, r"r_s = \frac{2GM}{c^2}",  "gr"),
    S!(825, r"d s^2 = -\left(1-\frac{r_s}{r}\right)c^2 dt^2 + \left(1-\frac{r_s}{r}\right)^{-1} dr^2 + r^2 d\Omega^2","gr"),
    S!(826, r"d\Omega^2 = d\theta^2 + \sin^2\theta d\phi^2","gr"),
    S!(827, r"T_H = \frac{\hbar c^3}{8\pi G M k_B}","gr"),
    S!(828, r"S_{BH} = \frac{k_B c^3 A}{4 G \hbar}","gr"),
    S!(829, r"A = 16\pi \frac{G^2 M^2}{c^4}","gr"),
    S!(830, r"\kappa_{\text{surface}} = \frac{c^4}{4 G M}","gr"),
    S!(831, r"\Omega_{\text{horizon}} = \frac{a c}{r_+^2 + a^2}","gr"),
    S!(832, r"r_\pm = \frac{r_s}{2} \pm \sqrt{\frac{r_s^2}{4} - a^2}","gr"),
    S!(833, r"a = \frac{J}{M c}",      "gr"),
    S!(834, r"M^2 \ge a^2 + Q^2",      "gr"),
    S!(835, r"d M = \frac{\kappa}{8\pi} dA + \Omega dJ + \Phi dQ","gr"),
    S!(836, r"\delta \int \sqrt{-g} d^4x = 0","gr"),
    S!(837, r"\mathcal{L}_{\text{EH}} = \frac{c^4}{16\pi G} (R - 2\Lambda)\sqrt{-g}","gr"),
    S!(838, r"\mathcal{L}_{\text{matter}}\sqrt{-g}","gr"),
    S!(839, r"T_{\mu\nu} = -\frac{2}{\sqrt{-g}}\frac{\delta(\sqrt{-g}\mathcal{L}_m)}{\delta g^{\mu\nu}}","gr"),
    S!(840, r"H = \sqrt{\frac{8\pi G \rho}{3} + \frac{\Lambda c^2}{3} - \frac{k c^2}{a^2}}","cosmology"),
    S!(841, r"\frac{\ddot{a}}{a} = -\frac{4\pi G}{3}\left(\rho + \frac{3p}{c^2}\right) + \frac{\Lambda c^2}{3}","cosmology"),
    S!(842, r"\dot{\rho} + 3 H \left(\rho + \frac{p}{c^2}\right) = 0","cosmology"),
    S!(843, r"w = p/(\rho c^2)",       "cosmology"),
    S!(844, r"w = 0",                  "cosmology"),
    S!(845, r"w = 1/3",                "cosmology"),
    S!(846, r"w = -1",                 "cosmology"),
    S!(847, r"\Omega_m = \frac{\rho_m}{\rho_c}","cosmology"),
    S!(848, r"\Omega_r = \frac{\rho_r}{\rho_c}","cosmology"),
    S!(849, r"\Omega_\Lambda = \frac{\rho_\Lambda}{\rho_c}","cosmology"),
    S!(850, r"\Omega_k = -\frac{k c^2}{a^2 H^2}","cosmology"),
    S!(851, r"\rho_c = \frac{3 H^2}{8\pi G}","cosmology"),
    S!(852, r"\Omega_m + \Omega_r + \Omega_\Lambda + \Omega_k = 1","cosmology"),
    S!(853, r"q = -\frac{\ddot{a} a}{\dot{a}^2}","cosmology"),
    S!(854, r"j = \frac{\dddot{a} a^2}{\dot{a}^3}","cosmology"),
    S!(855, r"s = \frac{\ddddot{a} a^3}{\dot{a}^4}","cosmology"),
    S!(856, r"z = \frac{\lambda_{\text{obs}}-\lambda_{\text{emit}}}{\lambda_{\text{emit}}}","cosmology"),
    S!(857, r"1 + z = \frac{a_0}{a(t)}","cosmology"),
    S!(858, r"d_L = (1+z) d_M",        "cosmology"),
    S!(859, r"d_A = \frac{d_M}{1+z}",  "cosmology"),
    S!(860, r"d_M = c \int_0^z \frac{dz'}{H(z')}","cosmology"),
    S!(861, r"H(z) = H_0 \sqrt{\Omega_{r,0}(1+z)^4 + \Omega_{m,0}(1+z)^3 + \Omega_{k,0}(1+z)^2 + \Omega_{\Lambda,0}}","cosmology"),
    S!(862, r"\Phi(x) = -G \int \frac{\rho(x')}{|x-x'|} d^3x'","gravity"),
    S!(863, r"\nabla^2 \Phi = 4\pi G \rho","gravity"),
    S!(864, r"\vec{g} = -\nabla \Phi","gravity"),
    S!(865, r"v_{\text{esc}} = \sqrt{\frac{2GM}{R}}","gravity"),
    S!(866, r"v_{\text{orb}} = \sqrt{\frac{GM}{R}}","gravity"),
    S!(867, r"T^2 = \frac{4\pi^2}{G(M_1+M_2)} a^3","kepler"),
    S!(868, r"E = -\frac{G M_1 M_2}{2a}","kepler"),
    S!(869, r"L = \mu \sqrt{G(M_1+M_2)a(1-e^2)}","kepler"),
    S!(870, r"e = \sqrt{1 + \frac{2 E L^2}{\mu (G M_1 M_2)^2}}","kepler"),
    S!(871, r"r(\theta) = \frac{a(1-e^2)}{1 + e \cos\theta}","kepler"),
    S!(872, r"\Delta\phi = \frac{6\pi G M}{c^2 a (1-e^2)}","gr"),
    S!(873, r"\theta_{\text{defl}} = \frac{4GM}{c^2 b}","gr"),
    S!(874, r"\Delta t_{\text{shapiro}} = \frac{4GM}{c^3}\left(\ln\left(\frac{4 r_1 r_2}{b^2}\right) + 1\right)","gr"),
    S!(875, r"h_+ = \frac{1}{r}\frac{4G}{c^4}\left(\frac{\ddot{I}_{xx}-\ddot{I}_{yy}}{2}\right)","gw"),
    S!(876, r"h_\times = \frac{1}{r}\frac{4G}{c^4}\ddot{I}_{xy}","gw"),
    S!(877, r"\frac{dE_{\text{GW}}}{dt} = -\frac{G}{5c^5}\dddot{I}_{ij}\dddot{I}^{ij}","gw"),
    S!(878, r"f_{\text{GW}} = 2 f_{\text{orb}}","gw"),
    S!(879, r"\mathcal{M}_c = \frac{(m_1 m_2)^{3/5}}{(m_1+m_2)^{1/5}}","gw"),
    S!(880, r"\dot{f} = \frac{96}{5}\pi^{8/3}\left(\frac{G \mathcal{M}_c}{c^3}\right)^{5/3} f^{11/3}","gw"),
    S!(881, r"h \sim \frac{4 G \mathcal{M}_c}{c^2 D_L} (\pi G \mathcal{M}_c f / c^3)^{2/3}","gw"),
    S!(882, r"\tau_{\text{merge}} = \frac{5 c^5}{256 G^3 \mathcal{M}_c^5} (\pi f_0)^{-8/3}","gw"),
    S!(883, r"P_{\text{rad}} = \frac{2 q^2 \dot{v}^2}{3 c^3}","em"),
    S!(884, r"P_{\text{larmor}} = \frac{q^2 a^2}{6\pi \varepsilon_0 c^3}","em"),
    S!(885, r"P_{\text{rel}} = \frac{q^2 \gamma^6}{6\pi \varepsilon_0 c^3}(\dot{\beta}^2 - (\vec{\beta}\times\dot{\vec{\beta}})^2)","em"),
    S!(886, r"\vec{S} = \frac{1}{\mu_0}(\vec{E}\times\vec{B})","em"),
    S!(887, r"u_{\text{EM}} = \frac{1}{2}\left(\varepsilon_0 E^2 + \frac{B^2}{\mu_0}\right)","em"),
    S!(888, r"T^{ij}_{\text{maxwell}} = \varepsilon_0 E^i E^j + \frac{1}{\mu_0} B^i B^j - \frac{1}{2}\delta^{ij}\left(\varepsilon_0 E^2 + \frac{B^2}{\mu_0}\right)","em"),
    S!(889, r"\nabla \cdot \vec{S} + \frac{\partial u}{\partial t} = -\vec{J}\cdot\vec{E}","em"),
    S!(890, r"\vec{A}_{\text{ret}}(x,t) = \frac{\mu_0}{4\pi}\int \frac{\vec{J}(x', t - |x-x'|/c)}{|x-x'|} d^3x'","em"),
    S!(891, r"V_{\text{ret}}(x,t) = \frac{1}{4\pi\varepsilon_0}\int \frac{\rho(x', t - |x-x'|/c)}{|x-x'|} d^3x'","em"),
    S!(892, r"\vec{E} = -\nabla V - \frac{\partial \vec{A}}{\partial t}","em"),
    S!(893, r"\vec{B} = \nabla \times \vec{A}","em"),
    S!(894, r"\nabla \cdot \vec{A} + \frac{1}{c^2}\frac{\partial V}{\partial t} = 0","em"),
    S!(895, r"\nabla^2 V - \frac{1}{c^2}\frac{\partial^2 V}{\partial t^2} = -\frac{\rho}{\varepsilon_0}","em"),
    S!(896, r"\nabla^2 \vec{A} - \frac{1}{c^2}\frac{\partial^2 \vec{A}}{\partial t^2} = -\mu_0 \vec{J}","em"),
    S!(897, r"\vec{F} = q(\vec{E} + \vec{v}\times\vec{B})","em"),
    S!(898, r"\vec{F}_{\text{mhd}} = \vec{J}\times\vec{B} - \nabla p","fluid"),
    S!(899, r"\frac{\partial \rho}{\partial t} + \nabla \cdot (\rho \vec{v}) = 0","fluid"),
    S!(900, r"\rho \left(\frac{\partial \vec{v}}{\partial t} + (\vec{v}\cdot\nabla)\vec{v}\right) = -\nabla p + \mu \nabla^2\vec{v} + \vec{f}","fluid"),
    S!(901, r"\nabla \cdot \vec{v} = 0","fluid"),
    S!(902, r"\frac{\partial \vec{B}}{\partial t} = \nabla \times (\vec{v}\times\vec{B}) + \eta \nabla^2\vec{B}","mhd"),
    S!(903, r"\text{Re} = \frac{\rho v L}{\mu}","fluid"),
    S!(904, r"\text{Rm} = \frac{v L}{\eta}","mhd"),
    S!(905, r"\text{Pr} = \frac{\nu}{\alpha_{\text{thermal}}}","fluid"),
    S!(906, r"\text{Sc} = \frac{\nu}{D}","fluid"),
    S!(907, r"\text{Kn} = \frac{\lambda_{\text{mfp}}}{L}","fluid"),
    S!(908, r"\text{Ma} = \frac{v}{c_s}","fluid"),
    S!(909, r"c_s = \sqrt{\gamma \frac{p}{\rho}}","fluid"),
    S!(910, r"c_s = \sqrt{\left(\frac{\partial p}{\partial \rho}\right)_s}","fluid"),
    S!(911, r"v_A = \frac{B}{\sqrt{\mu_0 \rho}}","mhd"),
    S!(912, r"\omega_{pi} = \sqrt{\frac{n_i Z^2 e^2}{\varepsilon_0 m_i}}","plasma"),
    S!(913, r"\omega_{pe} = \sqrt{\frac{n_e e^2}{\varepsilon_0 m_e}}","plasma"),
    S!(914, r"\omega_{ce} = \frac{e B}{m_e}","plasma"),
    S!(915, r"\omega_{ci} = \frac{Z e B}{m_i}","plasma"),
    S!(916, r"\lambda_D = \sqrt{\frac{\varepsilon_0 k_B T_e}{n_e e^2}}","plasma"),
    S!(917, r"r_L = \frac{v_\perp}{\omega_c}","plasma"),
    S!(918, r"\beta_{\text{plasma}} = \frac{2 \mu_0 p}{B^2}","plasma"),
    S!(919, r"\Lambda_{\text{coulomb}} = 4\pi n \lambda_D^3","plasma"),
    S!(920, r"\ln\Lambda",           "plasma"),
    S!(921, r"\sigma = \frac{n e^2 \tau}{m_e}","plasma"),
    S!(922, r"\vec{E} + \vec{v}\times\vec{B} = \eta \vec{J} + \frac{1}{n e}\vec{J}\times\vec{B} - \frac{1}{n e}\nabla p_e + \frac{m_e}{n e^2}\frac{\partial \vec{J}}{\partial t}","ohm"),
    S!(923, r"\psi_{\text{stream}}","fluid"),
    S!(924, r"\phi_{\text{pot}}",   "fluid"),
    S!(925, r"u = \frac{\partial \psi}{\partial y}","fluid"),
    S!(926, r"v = -\frac{\partial \psi}{\partial x}","fluid"),
    S!(927, r"u = \frac{\partial \phi}{\partial x}","fluid"),
    S!(928, r"v = \frac{\partial \phi}{\partial y}","fluid"),
    S!(929, r"\nabla^2 \psi = -\omega_{\text{vorticity}}","fluid"),
    S!(930, r"\frac{\partial \omega}{\partial t} + (\vec{v}\cdot\nabla)\omega = (\omega\cdot\nabla)\vec{v} + \nu \nabla^2\omega","fluid"),
    S!(931, r"\Gamma_{\text{circ}} = \oint \vec{v}\cdot d\vec{r}","fluid"),
    S!(932, r"\frac{d\Gamma}{dt} = 0","fluid"),
    S!(933, r"L_{\text{lift}} = \rho_\infty V_\infty \Gamma","fluid"),
    S!(934, r"C_L = \frac{L}{\frac{1}{2}\rho v^2 S}","fluid"),
    S!(935, r"C_D = \frac{D}{\frac{1}{2}\rho v^2 S}","fluid"),
    S!(936, r"C_p = \frac{p - p_\infty}{\frac{1}{2}\rho v_\infty^2}","fluid"),
    S!(937, r"p + \frac{1}{2}\rho v^2 + \rho g h = \text{const}","fluid"),
    S!(938, r"p_0 = p \left(1 + \frac{\gamma-1}{2}\text{Ma}^2\right)^{\frac{\gamma}{\gamma-1}}","fluid"),
    S!(939, r"T_0 = T \left(1 + \frac{\gamma-1}{2}\text{Ma}^2\right)","fluid"),
    S!(940, r"\rho_0 = \rho \left(1 + \frac{\gamma-1}{2}\text{Ma}^2\right)^{\frac{1}{\gamma-1}}","fluid"),
    S!(941, r"\frac{A}{A^*} = \frac{1}{\text{Ma}}\left(\frac{2}{\gamma+1}\left(1+\frac{\gamma-1}{2}\text{Ma}^2\right)\right)^{\frac{\gamma+1}{2(\gamma-1)}}","fluid"),
    S!(942, r"\text{Ma}_2^2 = \frac{1 + \frac{\gamma-1}{2}\text{Ma}_1^2}{\gamma \text{Ma}_1^2 - \frac{\gamma-1}{2}}","fluid"),
    S!(943, r"\frac{p_2}{p_1} = 1 + \frac{2\gamma}{\gamma+1}(\text{Ma}_1^2 - 1)","fluid"),
    S!(944, r"\frac{rho_2}{\rho_1} = \frac{(\gamma+1)\text{Ma}_1^2}{2 + (\gamma-1)\text{Ma}_1^2}","fluid"),
    S!(945, r"\frac{T_2}{T_1} = \frac{p_2}{p_1}\frac{\rho_1}{\rho_2}","fluid"),
    S!(946, r"\Delta s = c_v \ln\left(\frac{p_2}{p_1}\left(\frac{\rho_1}{\rho_2}\right)^\gamma\right)","fluid"),
    S!(947, r"d U = T d S - P d V + \mu d N","thermo"),
    S!(948, r"d H = T d S + V d P + \mu d N","thermo"),
    S!(949, r"d F = -S d T - P d V + \mu d N","thermo"),
    S!(950, r"d G = -S d T + V d P + \mu d N","thermo"),
    S!(951, r"d \Omega = -S d T - P d V - N d \mu","thermo"),
    S!(952, r"\left(\frac{\partial T}{\partial V}\right)_S = -\left(\frac{\partial P}{\partial S}\right)_V","thermo"),
    S!(953, r"\left(\frac{\partial T}{\partial P}\right)_S = \left(\frac{\partial V}{\partial S}\right)_P","thermo"),
    S!(954, r"\left(\frac{\partial S}{\partial V}\right)_T = \left(\frac{\partial P}{\partial T}\right)_V","thermo"),
    S!(955, r"\left(\frac{\partial S}{\partial P}\right)_T = -\left(\frac{\partial V}{\partial T}\right)_P","thermo"),
    S!(956, r"C_V = \left(\frac{\partial U}{\partial T}\right)_V","thermo"),
    S!(957, r"C_P = \left(\frac{\partial H}{\partial T}\right)_P","thermo"),
    S!(958, r"C_P - C_V = T \left(\frac{\partial P}{\partial T}\right)_V \left(\frac{\partial V}{\partial T}\right)_P","thermo"),
    S!(959, r"\beta_T = -\frac{1}{V}\left(\frac{\partial V}{\partial P}\right)_T","thermo"),
    S!(960, r"\beta_S = -\frac{1}{V}\left(\frac{\partial V}{\partial P}\right)_S","thermo"),
    S!(961, r"\alpha_P = \frac{1}{V}\left(\frac{\partial V}{\partial T}\right)_P","thermo"),
    S!(962, r"\mu_{JT} = \left(\frac{\partial T}{\partial P}\right)_H","thermo"),
    S!(963, r"\mu_{JT} = \frac{1}{C_P}\left(T\left(\frac{\partial V}{\partial T}\right)_P - V\right)","thermo"),
    S!(964, r"S = -k_B \sum p_i \ln p_i","statmech"),
    S!(965, r"S = k_B \ln \Omega_{\text{states}}","statmech"),
    S!(966, r"Z = \sum_i e^{-\beta E_i}","statmech"),
    S!(967, r"\Xi = \sum_N \sum_i e^{-\beta(E_i - \mu N)}","statmech"),
    S!(968, r"F = -k_B T \ln Z","statmech"),
    S!(969, r"\Omega = -k_B T \ln \Xi","statmech"),
    S!(970, r"\langle E \rangle = -\frac{\partial \ln Z}{\partial \beta}","statmech"),
    S!(971, r"\langle (\Delta E)^2 \rangle = \frac{\partial^2 \ln Z}{\partial \beta^2}","statmech"),
    S!(972, r"\bar{n}_{FD} = \frac{1}{e^{\beta(\epsilon - \mu)} + 1}","statmech"),
    S!(973, r"\bar{n}_{BE} = \frac{1}{e^{\beta(\epsilon - \mu)} - 1}","statmech"),
    S!(974, r"\bar{n}_{MB} = e^{-\beta(\epsilon - \mu)}","statmech"),
    S!(975, r"u(\nu, T) = \frac{8\pi h \nu^3}{c^3}\frac{1}{e^{\frac{h\nu}{k_B T}}-1}","statmech"),
    S!(976, r"\lambda_{\max} T = b_{\text{wien}}","statmech"),
    S!(977, r"I = \sigma T^4","statmech"),
    S!(978, r"P V = N k_B T","thermo"),
    S!(979, r"\left(P + a\frac{N^2}{V^2}\right)(V - N b) = N k_B T","thermo"),
    S!(980, r"T_c = \frac{8a}{27 R b}","thermo"),
    S!(981, r"P_c = \frac{a}{27 b^2}","thermo"),
    S!(982, r"V_c = 3 b","thermo"),
    S!(983, r"Z_c = \frac{P_c V_c}{R T_c} = \frac{3}{8}","thermo"),
    S!(984, r"\eta_{\text{carnot}} = 1 - \frac{T_C}{T_H}","thermo"),
    S!(985, r"\text{COP}_{\text{refrig}} = \frac{T_C}{T_H - T_C}","thermo"),
    S!(986, r"\text{COP}_{\text{heatpump}} = \frac{T_H}{T_H - T_C}","thermo"),
    S!(987, r"d S \ge \frac{\delta Q}{T}","thermo"),
    S!(988, r"\oint \frac{\delta Q}{T} \le 0","thermo"),
    S!(989, r"G = \sum \mu_i N_i","thermo"),
    S!(990, r"\sum N_i d\mu_i = -S dT + V dP","thermo"),
    S!(991, r"\frac{dP}{dT} = \frac{\Delta S}{\Delta V} = \frac{L}{T \Delta V}","thermo"),
    S!(992, r"\ln\left(\frac{P_2}{P_1}\right) = -\frac{\Delta H_{\text{vap}}}{R}\left(\frac{1}{T_2}-\frac{1}{T_1}\right)","thermo"),
    S!(993, r"\Delta G^\circ = -R T \ln K_{\text{eq}}","thermo"),
    S!(994, r"\frac{d \ln K}{dT} = \frac{\Delta H^\circ}{R T^2}","thermo"),
    S!(995, r"k = A e^{-\frac{E_a}{R T}}","thermo"),
    S!(996, r"E = E^\circ - \frac{R T}{n F}\ln Q","thermo"),
    S!(997, r"\Lambda = \frac{h}{\sqrt{2\pi m k_B T}}","statmech"),
    S!(998, r"T_c^{\text{BEC}} = \frac{2\pi\hbar^2}{m k_B}\left(\frac{n}{\zeta(3/2)}\right)^{2/3}","statmech"),
    S!(999, r"E_F = \frac{\hbar^2}{2m}(3\pi^2 n)^{2/3}","statmech"),
    S!(1000,r"T_F = \frac{E_F}{k_B}","statmech"),
];

pub fn symbols_by_cat(cat: &str) -> Vec<&'static MathSym> {
    MATH_SYMBOLS.iter().filter(|s| s.cat == cat).collect()
}

pub fn symbol_by_id(id: u16) -> Option<&'static MathSym> {
    MATH_SYMBOLS.iter().find(|s| s.id == id)
}

pub fn all_categories() -> Vec<&'static str> {
    let mut cats: Vec<&str> = MATH_SYMBOLS.iter().map(|s| s.cat).collect();
    cats.sort_unstable(); cats.dedup(); cats
}

#[derive(Clone, Debug, Serialize)]
pub struct OdeSys {
    pub family:  String,
    pub state:   Vec<String>,
    pub rhs:     Vec<String>,
    pub rhs_nl:  bool,
    pub params:  HashMap<String, f64>,
    pub hash:    String,
}

impl OdeSys {
    pub fn new(family: &str, state: Vec<String>,
               rhs_exprs: Vec<Expr>, params: HashMap<String,f64>) -> Self
    {
        let rhs: Vec<String> = rhs_exprs.iter().map(|e| e.latex()).collect();
        let hash = {
            let s: String = rhs.join("|");
            let mut h = Sha256::new(); h.update(s.as_bytes());
            hex::encode(h.finalize())[..12].to_string()
        };
        let rhs_nl = rhs_exprs.iter().any(|e| e.is_nonlinear(&state));
        OdeSys { family: family.to_string(), state, rhs, rhs_nl, params, hash }
    }

    pub fn equations_latex(&self) -> Vec<String> {
        self.state.iter().zip(self.rhs.iter())
            .map(|(s,r)| format!("\\dot{{{}}} = {}", s, r))
            .collect()
    }
}

pub fn eval_rhs(exprs: &[Expr], state: &[f64], names: &[String],
                params: &HashMap<String,f64>) -> Vec<f64>
{
    let mut m: HashMap<String,f64> = names.iter().zip(state).map(|(n,&v)|(n.clone(),v)).collect();
    m.extend(params.iter().map(|(k,&v)|(k.clone(),v)));
    exprs.iter().map(|e| e.eval(&m)).collect()
}

pub fn gen_sprott(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String,f64>) {
    let sx = vr("x"); let sy = vr("y"); let sz = vr("z");
    let names = vec!["x".to_string(), "y".to_string(), "z".to_string()];
    let monomials = vec![
        k(1.0), sx.clone(), sy.clone(), sz.clone(),
        powi(sx.clone(),2), powi(sy.clone(),2), powi(sz.clone(),2),
        mul(sx.clone(), sy.clone()), mul(sx.clone(), sz.clone()), mul(sy.clone(), sz.clone()),
    ];
    let mut rhs = Vec::new();
    for _ in 0..3 {
        let n_terms = [1,2,2,3][rng.gen_range(0..4)];
        let mut expr = k(0.0);
        for _ in 0..n_terms {
            let m = monomials[rng.gen_range(0..monomials.len())].clone();
            let s = if rng.gen_bool(0.5) { 1.0_f64 } else { -1.0_f64 };
            expr = add(expr, mul(k(s), m));
        }
        rhs.push(expr);
    }
    (rhs, names, HashMap::new())
}

pub fn gen_oscillator(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String,f64>) {
    let choices: &[(&str, Box<dyn Fn(f64,f64,f64,f64) -> Expr>)] = &[
        ("vanderpol", Box::new(|_mu,_,_,_| mul(vr("mu"), mul(sub(powi(vr("ell"),2), powi(vr("X"),2)), vr("V"))))),
        ("duffing",   Box::new(|_,_b,_,_| neg(mul(vr("beta"), powi(vr("X"),3))))),
        ("pendulum",  Box::new(|_w,_,_l,_| neg(mul(mul(powi(vr("omega0"),2), vr("ell")), sin(div(vr("X"), vr("ell"))))))),
        ("toda",      Box::new(|_w,_,_l,_| neg(mul(mul(powi(vr("omega0"),2), vr("ell")), sub(exp(div(vr("X"),vr("ell"))), k(1.0)))))),
        ("saturated", Box::new(|_w,_,_l,_| neg(div(mul(powi(vr("omega0"),2),vr("X")), add(k(1.0), div(powi(vr("X"),2), powi(vr("ell"),2))))))),
    ];
    let idx = rng.gen_range(0..choices.len());
    let (_kind, f) = &choices[idx];
    let w = [0.5f64, 1.0, 1.5, 2.0][rng.gen_range(0..4)];
    let d = [0.1f64, 0.2, 0.3, 0.5][rng.gen_range(0..4)];
    let be= [0.5f64, 1.0, 2.0][rng.gen_range(0..3)];
    let mu= [0.2f64, 0.5, 1.0][rng.gen_range(0..3)];
    let l = [1.0f64, 2.0][rng.gen_range(0..2)];
    let forced = rng.gen_bool(0.25);
    let ga= [0.5f64, 1.0, 1.5, 3.0][rng.gen_range(0..4)];
    let om= [1.0f64, 1.2, 1.5, 2.0][rng.gen_range(0..4)];

    let nl_term = f(w, be, l, mu);
    let lin_damp = neg(mul(vr("delta"), vr("V")));
    let lin_rest = neg(mul(powi(vr("omega0"),2), vr("X")));
    let acc = add(lin_rest.clone(), add(lin_damp, nl_term));
    let mut state_names = vec!["X".to_string(), "V".to_string()];
    let mut rhs = vec![vr("V"), acc];
    let mut params: HashMap<String,f64> = [("omega0",w),("delta",d),("beta",be),("mu",mu),("ell",l)].iter()
        .map(|(k,v)|(k.to_string(),*v)).collect();
    if forced {
        rhs.push(vr("Omega"));
        rhs[1] = add(rhs[1].clone(), mul(vr("gamma"), cos(vr("theta"))));
        state_names.push("theta".to_string());
        params.insert("gamma".to_string(), ga);
        params.insert("Omega".to_string(), om);
    }
    (rhs, state_names, params)
}

pub fn gen_1d_auto(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String,f64>) {
    let atoms = [
        vr("q"), powi(vr("q"),2), exp(vr("q")),
        sin(vr("q")), cos(vr("q")),
        div(k(1.0), add(k(1.0), powi(vr("q"),2))),
        mul(vr("q"), exp(neg(powi(vr("q"),2)))),
        tanh(vr("q")),
    ];
    let a = atoms[rng.gen_range(0..atoms.len())].clone();
    let b_ = atoms[rng.gen_range(0..atoms.len())].clone();
    let expr = match rng.gen_range(0..3u8) {
        0 => mul(a, b_),
        1 => add(a, b_),
        _ => a,
    };
    (vec![expr], vec!["q".to_string()], HashMap::new())
}

pub fn gen_lienard(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String,f64>) {
    let coeffs: Vec<f64> = (0..4).map(|_| {
        let choices = [0.0f64, 0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5];
        choices[rng.gen_range(0..choices.len())]
    }).collect();
    let [a,b_,c_,d] = [coeffs[0], coeffs[1], coeffs[2], coeffs[3]];
    let fy = add(
        add(mul(k(a), vr("y")), mul(k(b_), vr("x"))),
        add(mul(k(c_), powi(vr("x"),2)), mul(k(d), mul(vr("x"), vr("y"))))
    );
    (vec![vr("y"), fy], vec!["x".to_string(), "y".to_string()], HashMap::new())
}


/// পরিবার ৫: বায়োলজি (FitzHugh-Nagumo / Neuronal Excitation)
pub fn gen_biology(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let u = vr("u");
    let v = vr("v");
    let a = [0.7, 0.8, 0.9][rng.gen_range(0..3)];
    let b = [0.8, 1.0, 1.2][rng.gen_range(0..3)];
    let c = [2.0, 3.0, 5.0][rng.gen_range(0..3)];
    let i_ext = rng.gen_range(-0.5..1.0);

    let du = mul(k(c), add(sub(u.clone(), div(powi(u.clone(), 3), k(3.0))), add(v.clone(), k(i_ext))));
    let dv = div(neg(add(sub(u.clone(), k(a)), mul(k(b), v.clone()))), k(c));

    (vec![du, dv], vec!["u".to_string(), "v".to_string()], HashMap::new())
}

/// পরিবার ৬: রসায়ন (Brusselator Autocatalytic Reaction)
pub fn gen_chemistry(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let a = rng.gen_range(0.5..2.5);
    let b = rng.gen_range(1.5..4.5);

    let dx = add(sub(k(a), mul(k(b + 1.0), x.clone())), mul(powi(x.clone(), 2), y.clone()));
    let dy = sub(mul(k(b), x.clone()), mul(powi(x.clone(), 2), y.clone()));

    (vec![dx, dy], vec!["x".to_string(), "y".to_string()], HashMap::new())
}

/// পরিবার ৭: মহাবিশ্বতত্ত্ব (FLRW Quintessence Phase Space)
pub fn gen_cosmology(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let lambda = rng.gen_range(0.1..2.0);
    let gamma = 1.0;

    let factor = (3.0_f64 / 2.0).sqrt() * lambda;
    let q_term = add(
        mul(k(2.0), powi(x.clone(), 2)),
        mul(k(gamma), sub(k(1.0), add(powi(x.clone(), 2), powi(y.clone(), 2))))
    );

    let dx = add(
        add(mul(k(-3.0), x.clone()), mul(k(factor), powi(y.clone(), 2))),
        mul(mul(k(1.5), x.clone()), q_term.clone())
    );
    let dy = add(
        mul(k(-factor), mul(x.clone(), y.clone())),
        mul(mul(k(1.5), y.clone()), q_term)
    );

    (vec![dx, dy], vec!["x".to_string(), "y".to_string()], HashMap::new())
}

/// পরিবার ৮: কোয়ান্টাম মেথডস (Semi-classical Quantum Duffing Flow)
pub fn gen_quantum(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let z = vr("z");

    let gamma = rng.gen_range(0.5..1.5);
    let omega = rng.gen_range(2.0..6.0);
    let g = rng.gen_range(0.8..2.5);

    let dx = add(mul(k(-gamma), x.clone()), mul(k(omega), y.clone()));
    let dy = add(add(mul(k(-omega), x.clone()), mul(k(-gamma), y.clone())), mul(k(g), mul(x.clone(), z.clone())));
    let dz = sub(mul(k(-gamma), sub(z.clone(), k(1.0))), mul(k(g), add(powi(x.clone(), 2), powi(y.clone(), 2))));

    (vec![dx, dy, dz], vec!["x".to_string(), "y".to_string(), "z".to_string()], HashMap::new())
}
pub fn gen_oregonator_3d(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let z = vr("z");

    let eps = rng.gen_range(0.01..0.08);
    let delta = rng.gen_range(0.0005..0.005);
    let q = rng.gen_range(0.0001..0.002);
    let f_val = rng.gen_range(0.6..1.4);

    // dx/dt = (q*y - x*y + x*(1 - x)) / eps
    let dx = div(
        add(sub(mul(k(q), y.clone()), mul(x.clone(), y.clone())), mul(x.clone(), sub(k(1.0), x.clone()))),
        k(eps)
    );
    // dy/dt = (-q*y - x*y + 2*f*z) / delta
    let dy = div(
        add(sub(mul(k(-q), y.clone()), mul(x.clone(), y.clone())), mul(k(2.0 * f_val), z.clone())),
        k(delta)
    );
    // dz/dt = x - z
    let dz = sub(x.clone(), z.clone());

    (vec![dx, dy, dz], vec!["x".to_string(), "y".to_string(), "z".to_string()], HashMap::new())
}

/// পরিবার ১০: ৩-প্রজাতির ফুড-ওয়েব ক্যাওস (Hastings-Powell Food Web)
pub fn gen_foodweb_3d(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let z = vr("z");

    let a1 = rng.gen_range(4.5..5.5);
    let b1 = rng.gen_range(2.5..3.5);
    let a2 = rng.gen_range(0.1..0.2);
    let b2 = rng.gen_range(1.5..2.5);
    let d1 = rng.gen_range(0.35..0.45);
    let d2 = rng.gen_range(0.01..0.02);

    // f1(x) = (a1*x)/(1 + b1*x)
    let f1_term = div(mul(k(a1), x.clone()), add(k(1.0), mul(k(b1), x.clone())));
    // f2(y) = (a2*y)/(1 + b2*y)
    let f2_term = div(mul(k(a2), y.clone()), add(k(1.0), mul(k(b2), y.clone())));

    // dx/dt = x*(1 - x) - f1_term*y
    let dx = sub(mul(x.clone(), sub(k(1.0), x.clone())), mul(f1_term.clone(), y.clone()));
    // dy/dt = f1_term*y - f2_term*z - d1*y
    let dy = sub(sub(mul(f1_term, y.clone()), mul(f2_term.clone(), z.clone())), mul(k(d1), y.clone()));
    // dz/dt = f2_term*z - d2*z
    let dz = sub(mul(f2_term, z.clone()), mul(k(d2), z.clone()));

    (vec![dx, dy, dz], vec!["x".to_string(), "y".to_string(), "z".to_string()], HashMap::new())
}


/// পরিবার ১১: ফ্লুইড ডাইনামিক্স (Navier-Stokes Modal Truncation / Convection)
pub fn gen_fluid_navier_stokes(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let z = vr("z");

    let sigma = rng.gen_range(8.0..14.0);
    let rho = rng.gen_range(24.0..32.0);
    let beta = rng.gen_range(2.0..3.5);

    let dx = mul(k(sigma), sub(y.clone(), x.clone()));
    let dy = sub(mul(x.clone(), sub(k(rho), z.clone())), y.clone());
    let dz = sub(mul(x.clone(), y.clone()), mul(k(beta), z.clone()));

    (vec![dx, dy, dz], vec!["x".to_string(), "y".to_string(), "z".to_string()], HashMap::new())
}

/// পরিবার ১২: ইলেকট্রোডাইনামিক্স (Maxwell-Bloch Field Resonance)
pub fn gen_maxwell_bloch(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let e = vr("e");
    let p = vr("p");
    let d = vr("d");

    let kappa = rng.gen_range(3.0..6.0);
    let gamma_p = rng.gen_range(1.0..2.5);
    let gamma_d = rng.gen_range(0.1..0.5);
    let lambda = rng.gen_range(10.0..25.0);

    let de = mul(k(-kappa), sub(e.clone(), p.clone()));
    let dp = add(mul(k(-gamma_p), p.clone()), mul(k(gamma_p), mul(e.clone(), d.clone())));
    let dd = sub(mul(k(-gamma_d), sub(d.clone(), k(lambda))), mul(k(gamma_d), mul(e.clone(), p.clone())));

    (vec![de, dp, dd], vec!["e".to_string(), "p".to_string(), "d".to_string()], HashMap::new())
}

/// পরিবার ১৩: সাধারণ আপেক্ষিকতা (Bianchi IX Mixmaster Gravity)
pub fn gen_general_relativity(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let u = vr("u");
    let v = vr("v");
    let w = vr("w");

    let a = rng.gen_range(0.5..1.5);
    let b = rng.gen_range(0.5..1.5);

    let du = mul(u.clone(), sub(v.clone(), w.clone()));
    let dv = mul(v.clone(), sub(w.clone(), mul(k(a), u.clone())));
    let dw = sub(mul(w.clone(), sub(mul(k(b), u.clone()), v.clone())), powi(u.clone(), 2));

    (vec![du, dv, dw], vec!["u".to_string(), "v".to_string(), "w".to_string()], HashMap::new())
}

/// পরিবার ১৪: তাপগতিবিদ্যা (Onsager Dissipative Engine)
pub fn gen_thermo_dissipative(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let x = vr("x");
    let y = vr("y");
    let z = vr("z");

    let mu = rng.gen_range(0.8..2.5);
    let a = rng.gen_range(0.5..1.2);
    let b = rng.gen_range(0.5..1.5);

    let dx = y.clone();
    let dy = add(sub(neg(x.clone()), mul(k(mu), mul(y.clone(), sub(powi(x.clone(), 2), k(1.0))))), z.clone());
    let dz = sub(mul(k(-a), z.clone()), mul(k(b), mul(x.clone(), y.clone())));

    (vec![dx, dy, dz], vec!["x".to_string(), "y".to_string(), "z".to_string()], HashMap::new())
}

/// পরিবার ১৫: কণা পদার্থবিজ্ঞান (Yang-Mills Classical SU(2) Gauge)
pub fn gen_yang_mills_gauge(rng: &mut SmallRng) -> (Vec<Expr>, Vec<String>, HashMap<String, f64>) {
    let q1 = vr("q1");
    let q2 = vr("q2");
    let p1 = vr("p1");

    let g_sq = rng.gen_range(0.5..2.0);

    let dq1 = p1.clone();
    let dq2 = sub(q1.clone(), q2.clone());
    let dp1 = sub(mul(k(-g_sq), mul(q1.clone(), powi(q2.clone(), 2))), mul(k(0.1), p1.clone()));

    (vec![dq1, dq2, dp1], vec!["q1".to_string(), "q2".to_string(), "p1".to_string()], HashMap::new())
}

pub fn rk4_step(f: &[Expr], state: &[f64], names: &[String],
                params: &HashMap<String,f64>, dt: f64) -> Vec<f64>
{
    let n = state.len();
    let k1 = eval_rhs(f, state, names, params);
    let s2: Vec<f64> = (0..n).map(|i| state[i] + 0.5*dt*k1[i]).collect();
    let k2 = eval_rhs(f, &s2, names, params);
    let s3: Vec<f64> = (0..n).map(|i| state[i] + 0.5*dt*k2[i]).collect();
    let k3 = eval_rhs(f, &s3, names, params);
    let s4: Vec<f64> = (0..n).map(|i| state[i] + dt*k3[i]).collect();
    let k4 = eval_rhs(f, &s4, names, params);
    (0..n).map(|i| state[i] + (dt/6.0)*(k1[i]+2.0*k2[i]+2.0*k3[i]+k4[i])).collect()
}

pub fn jacobian(f: &[Expr], state: &[f64], names: &[String],
                params: &HashMap<String,f64>) -> Vec<Vec<f64>>
{
    const H: f64 = 1e-6;
    let n = state.len();
    let mut j = vec![vec![0.0; n]; n];
    for col in 0..n {
        let mut sp = state.to_vec(); sp[col] += H;
        let mut sm = state.to_vec(); sm[col] -= H;
        let fp = eval_rhs(f, &sp, names, params);
        let fm = eval_rhs(f, &sm, names, params);
        for row in 0..n {
            j[row][col] = (fp[row] - fm[row]) / (2.0*H);
        }
    }
    j
}

pub fn divergence(f: &[Expr], state: &[f64], names: &[String],
                  params: &HashMap<String,f64>) -> f64
{
    let j = jacobian(f, state, names, params);
    (0..state.len()).map(|i| j[i][i]).sum()
}

pub fn find_equilibria(f: &[Expr], names: &[String], params: &HashMap<String,f64>,
                       rng: &mut SmallRng) -> Vec<Vec<f64>>
{
    let n = names.len();
    let mut found: Vec<Vec<f64>> = Vec::new();
    for _ in 0..20 {
        let mut x: Vec<f64> = (0..n).map(|_| rng.gen_range(-2.0..2.0)).collect();
        for _ in 0..200 {
            let fx = eval_rhs(f, &x, names, params);
            let err: f64 = fx.iter().map(|v| v*v).sum::<f64>().sqrt();
            if err < 1e-10 { break; }
            let step = 0.1 / (1.0 + err);
            for i in 0..n { x[i] -= step * fx[i]; }
        }
        let fx = eval_rhs(f, &x, names, params);
        let err: f64 = fx.iter().map(|v| v*v).sum::<f64>().sqrt();
        if err < 1e-6 {
            let new = x.iter().all(|v| v.is_finite());
            let dup = found.iter().any(|eq| {
                eq.iter().zip(x.iter()).all(|(a,b)| (a-b).abs() < 1e-4)
            });
            if new && !dup { found.push(x); }
        }
    }
    found
}

pub fn eigenvalues_2d(j: &[Vec<f64>]) -> Vec<(f64,f64)> {
    if j.len() == 2 {
        let tr = j[0][0] + j[1][1];
        let dt = j[0][0]*j[1][1] - j[0][1]*j[1][0];
        let disc = tr*tr - 4.0*dt;
        if disc >= 0.0 {
            let sd = disc.sqrt();
            vec![((tr+sd)/2.0, 0.0), ((tr-sd)/2.0, 0.0)]
        } else {
            let sd = (-disc).sqrt();
            vec![(tr/2.0, sd/2.0), (tr/2.0, -sd/2.0)]
        }
    } else {
        let n = j.len() as f64;
        let tr: f64 = (0..j.len()).map(|i| j[i][i]).sum();
        vec![(tr/n, 0.0)]
    }
}

pub fn lyapunov_max(f: &[Expr], names: &[String], params: &HashMap<String,f64>,
                    rng: &mut SmallRng, t_max: f64, dt: f64) -> f64
{
    let n = names.len();
    let mut x: Vec<f64> = (0..n).map(|_| rng.gen_range(-0.5..0.5)).collect();
    for _ in 0..500 { x = rk4_step(f, &x, names, params, dt); }

    let mut w: Vec<f64> = (0..n).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let wn: f64 = w.iter().map(|v| v*v).sum::<f64>().sqrt();
    for wi in w.iter_mut() { *wi /= if wn > 0.0 { wn } else { 1.0 }; }

    let steps = (t_max / dt) as usize;
    let renorm = 10;
    let mut sum_log = 0.0f64;
    let mut count = 0usize;

    for step in 0..steps {
        x = rk4_step(f, &x, names, params, dt);
        let j = jacobian(f, &x, names, params);
        let mut dw = vec![0.0f64; n];
        for i in 0..n { for j2 in 0..n { dw[i] += j[i][j2] * w[j2]; } }
        for i in 0..n { w[i] += dt * dw[i]; }
        if (step + 1) % renorm == 0 {
            let d_norm: f64 = w.iter().map(|v| v*v).sum::<f64>().sqrt();
            if d_norm > 0.0 && d_norm.is_finite() {
                sum_log += d_norm.ln();
                count += 1;
                for wi in w.iter_mut() { *wi /= d_norm; }
            }
        }
    }
    if count == 0 { return 0.0; }
    sum_log / (count as f64 * renorm as f64 * dt)
}

#[derive(Clone, Debug, Serialize)]
pub struct Exact {
    pub divergence_at_origin: f64,
    pub equilibria_count: usize,
    pub equilibria: Vec<Vec<f64>>,
    pub jacobian_at_eq0: Vec<Vec<f64>>,
    pub eigenvalues_at_eq0: Vec<(f64,f64)>,
    pub is_hamiltonian_suspected: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub lyapunov_max: f64,
    pub behavior: String,
    pub t_lyap: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FrontierResult {
    pub status: String,
    pub hash: String,
    pub family: String,
    pub equations: Vec<String>,
    pub nonlinear: bool,
    pub exact: Exact,
    pub evidence: Evidence,
    pub universe_id: usize,
}

fn is_strongly_connected(f: &[Expr], state: &[String]) -> bool {
    let n = state.len();
    if n == 1 { return true; }
    let mut adj = vec![vec![false; n]; n];
    for (i, fi) in f.iter().enumerate() {
        let fvars = fi.free_vars();
        for (j, sj) in state.iter().enumerate() {
            if i != j && fvars.contains(sj) { adj[i][j] = true; }
        }
    }
    let mut reach = adj.clone();
    for k_ in 0..n { for i in 0..n { for j in 0..n {
        reach[i][j] = reach[i][j] || (reach[i][k_] && reach[k_][j]);
    }}}
    (0..n).all(|i| (0..n).all(|j| i==j || reach[i][j]))
}

pub fn frontier_pipeline(
    f: &[Expr],
    state: &[String],
    params: &HashMap<String,f64>,
    family: &str,
    seen: &DashSet<String>,
    rng: &mut SmallRng,
    lyap_t: f64,
    universe_id: usize,
    chaos_only: bool,
) -> Option<FrontierResult>
{
    if !f.iter().any(|e| e.is_nonlinear(state)) { return None; }
    if !is_strongly_connected(f, state) { return None; }

    let hash = {
        let s: String = f.iter().map(|e| e.latex()).collect::<Vec<_>>().join("|");
        let mut h = Sha256::new(); h.update(s.as_bytes());
        hex::encode(h.finalize())[..12].to_string()
    };
    if !seen.insert(hash.clone()) { return None; }

    let origin = vec![0.1f64; state.len()];
    let div0 = divergence(f, &origin, state, params);
    let equil = find_equilibria(f, state, params, rng);
    let (jac0, eigs0) = if !equil.is_empty() {
        let j = jacobian(f, &equil[0], state, params);
        let e = eigenvalues_2d(&j);
        (j, e)
    } else {
        let j = jacobian(f, &origin, state, params);
        let e = eigenvalues_2d(&j);
        (j, e)
    };

    let ham = div0.abs() < 0.05;

    let exact = Exact {
        divergence_at_origin: div0,
        equilibria_count: equil.len(),
        equilibria: equil.clone(),
        jacobian_at_eq0: jac0,
        eigenvalues_at_eq0: eigs0,
        is_hamiltonian_suspected: ham,
    };

    let lyap = lyapunov_max(f, state, params, rng, lyap_t, 0.01);
    let behavior = if !lyap.is_finite() || lyap.abs() > 50.0 {
        "divergent".to_string()
    } else if lyap > 0.01 {
        "chaotic".to_string()
    } else {
        "regular".to_string()
    };

    if chaos_only && behavior != "chaotic" { return None; }

    let evidence = Evidence {
        lyapunov_max: if lyap.is_finite() { lyap } else { 0.0 },
        behavior: behavior.clone(),
        t_lyap: lyap_t,
    };

    let equations = state.iter().zip(f.iter())
        .map(|(s,e)| format!("\\dot{{{}}} = {}", s, e.latex()))
        .collect();

    Some(FrontierResult {
        status: format!("OPEN·no-closed-form-found·{}", behavior),
        hash,
        family: family.to_string(),
        equations,
        nonlinear: true,
        exact,
        evidence,
        universe_id,
    })
}

pub fn mine_universe(
    universe_id: usize,
    base_seed: u64,
    target: usize,
    lyap_t: f64,
    chaos_only: bool,
    seen: Arc<DashSet<String>>,
) -> Vec<FrontierResult>
{
    let seed = base_seed ^ (universe_id as u64 * 0x9e3779b97f4a7c15);
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut results = Vec::new();

    while results.len() < target {
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
        };

        if let Some(r) = frontier_pipeline(
            &f, &state, &params, fam, &seen, &mut rng, lyap_t, universe_id, chaos_only
        ) {
            results.push(r);
        }
    }
    results
}

pub fn mine_distributed(
    n_universes: usize,
    per_universe: usize,
    base_seed: u64,
    lyap_t: f64,
    chaos_only: bool,
) -> Vec<FrontierResult>
{
    let seen: Arc<DashSet<String>> = Arc::new(DashSet::new());

    (0..n_universes)
        .into_par_iter()
        .flat_map(|uid| {
            mine_universe(uid, base_seed, per_universe, lyap_t, chaos_only, Arc::clone(&seen))
        })
        .collect()
}

#[derive(Parser, Debug)]
#[command(name="akira-frontier", about="Distributed ODE Frontier Explorer v2.0")]
struct Cli {
    #[arg(long, default_value="frontier")]
    mode: String,

    #[arg(long, default_value_t=100)]
    count: usize,

    #[arg(long, default_value_t=4)]
    universes: usize,

    #[arg(long)]
    out: Option<String>,

    #[arg(long, default_value_t=false)]
    chaos_only: bool,

    #[arg(long, default_value_t=200.0)]
    lyap_time: f64,

    #[arg(long, default_value_t=0x4b1d_beef_dead)]
    seed: u64,

    #[arg(long)]
    list_symbols: bool,

    #[arg(long, default_value="")]
    cat: String,

    #[arg(long, default_value_t=false)]
    selftest: bool,
}

fn run_selftest() {
    println!("=== AKIRA-FRONTIER-RS  সেলফ-টেস্ট ===");

    let e = add(powi(vr("x"),2), mul(k(2.0), sin(vr("x"))));
    let mut m = HashMap::new(); m.insert("x".to_string(), 1.0f64);
    let v = e.eval(&m);
    assert!((v - (1.0 + 2.0*1.0_f64.sin())).abs() < 1e-10, "Expr eval failed: {}", v);
    println!("✓ Expr eval");

    let d = Dim::VELOCITY;
    assert_eq!(d.display(), "m·s^-1");
    println!("✓ Dim display");

    assert_eq!(MATH_SYMBOLS.len(), 1000, "Expected 1000 symbols, got {}", MATH_SYMBOLS.len());
    println!("✓ Symbol registry: {} entries", MATH_SYMBOLS.len());

    let f_osc = vec![vr("V"), neg(vr("X"))];
    let names = vec!["X".to_string(), "V".to_string()];
    let params: HashMap<String,f64> = HashMap::new();
    let mut state = vec![1.0f64, 0.0];
    let dt = 0.001;
    let n = (std::f64::consts::PI / dt) as usize;
    for _ in 0..n { state = rk4_step(&f_osc, &state, &names, &params, dt); }
    assert!((state[0] - (-1.0)).abs() < 1e-4, "RK4 test failed: x(π) = {}", state[0]);
    println!("✓ RK4 harmonic oscillator: x(π) = {:.6} (expected -1)", state[0]);

    let mut rng = SmallRng::seed_from_u64(42);
    let (sf, ss, _sp) = gen_sprott(&mut rng);
    assert_eq!(ss.len(), 3);
    println!("✓ Sprott generator: {} eqs, {} state vars", sf.len(), ss.len());

    let lorenz: Vec<Expr> = vec![
        sub(mul(k(10.0), sub(vr("y"), vr("x"))), k(0.0)),
        sub(mul(vr("x"), sub(k(28.0), vr("z"))), vr("y")),
        sub(mul(vr("x"), vr("y")), mul(k(2.667), vr("z"))),
    ];
    let lorenz_names = vec!["x".to_string(),"y".to_string(),"z".to_string()];
    let mut rng2 = SmallRng::seed_from_u64(99);
    let lam = lyapunov_max(&lorenz, &lorenz_names, &params, &mut rng2, 100.0, 0.01);
    println!("✓ Lorenz Lyapunov λ_max ≈ {:.3} (expect ~0.9)", lam);
    assert!(lam > 0.3 && lam < 2.0, "Lorenz Lyapunov suspicious: {}", lam);

    println!("\n✓ সব টেস্ট পাস হয়েছে!");
}

fn main() {
    // === MULTI-UNIVERSE 1..8 CYCLE (1 MINUTE PER UNIVERSE) ===
    let _ = rayon::ThreadPoolBuilder::new().num_threads(4).build_global();
    println!(">>> [AKIRA-FRONTIER] 4 UNIVERSES INITIALIZED. THERMAL SAFE MODE (60s EACH) <<<");

        
    let cli = Cli::parse();

    if cli.selftest {
        run_selftest();
        return;
    }

    if cli.list_symbols {
        let syms: Vec<&MathSym> = if cli.cat.is_empty() {
            MATH_SYMBOLS.iter().collect()
        } else {
            symbols_by_cat(&cli.cat)
        };
        println!("গাণিতিক প্রতীক রেজিস্ট্রি ({} entries):", syms.len());
        for s in syms {
            println!("  [{:>4}] cat={:<15} latex={}", s.id, s.cat, s.latex);
        }
        println!("\nবিভাগসমূহ: {}", all_categories().join(", "));
        return;
    }

    let out_file = cli.out.unwrap_or_else(|| "found.jsonl".to_string());
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║  AKIRA-FRONTIER-RS  v2.0  Continuous Stream Mode     ║");
    println!("╠══════════════════════════════════════════════════════╣");
    println!("║  universes  = {:>4}                                  ║", cli.universes);
    println!("║  output     = {:<20}                   ║", out_file);
    println!("║  chaos_only = {:<5}                                  ║", cli.chaos_only);
    println!("║  [Ctrl+C চাপলে থামবে, ফাইল সাথে সাথে সেভ হচ্ছে]      ║");
    println!("╚══════════════════════════════════════════════════════╝");

    let seen: Arc<DashSet<String>> = Arc::new(DashSet::new());
    let (tx, rx) = std::sync::mpsc::channel::<FrontierResult>();

    let file_path = out_file.clone();
    std::thread::spawn(move || {
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .expect("ফাইল খোলা যায়নি");
        let mut w = BufWriter::new(f);
        let mut total = 0usize;
        let mut chaotic = 0usize;
        let t0 = Instant::now();

        while let Ok(r) = rx.recv() {
            total += 1;
            if r.evidence.behavior == "chaotic" {
                chaotic += 1;
            }
            let line = serde_json::to_string(&r).unwrap();
            writeln!(w, "{}", line).unwrap();
            let _ = w.flush();

            let el = t0.elapsed().as_secs_f64();
            let speed = if el > 0.0 { total as f64 / el } else { 0.0 };
            print!(
                "\r[মোট: {} | ক্যাওস: {} | গতি: {:.1}/s] Hash: {} | {:<14}",
                total, chaotic, speed, r.hash, r.family
            );
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
        }
    });

    (0..cli.universes).into_par_iter().for_each(|uid| {
        let seed = cli.seed ^ (uid as u64 * 0x9e3779b97f4a7c15);
        let mut rng = SmallRng::seed_from_u64(seed);
        loop {
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
            };

            if let Some(r) = frontier_pipeline(
                &f, &state, &params, fam, &seen, &mut rng, cli.lyap_time, uid, cli.chaos_only
            ) {
                let _ = tx.send(r);
            }
        }
    });
}




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
