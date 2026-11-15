//! DEUDA TÉCNICA (E1-2b): conversor mínimo por interpolación lineal.
//! Se sustituye por `rubato` en el hilo decodificador del motor real.

pub struct Converter {
    ratio: f64, // frecuencia de entrada / frecuencia de salida
    in_ch: usize,
    out_ch: usize,
    pos: f64,       // posición fraccionaria; el índice -1 es el último frame del bloque anterior
    last: Vec<f32>, // último frame del bloque anterior
}

impl Converter {
    pub fn new(in_rate: u32, out_rate: u32, in_ch: usize, out_ch: usize) -> Result<Self, String> {
        if in_ch != out_ch && in_ch != 1 {
            return Err(format!("{in_ch} canales → {out_ch}: aún no soportado"));
        }
        Ok(Self {
            ratio: in_rate as f64 / out_rate as f64,
            in_ch,
            out_ch,
            pos: 0.0,
            last: vec![0.0; in_ch],
        })
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let in_ch = self.in_ch;
        let n = input.len() / in_ch;
        if n == 0 {
            return Vec::new();
        }

        let sample = |idx: isize, ch: usize| -> f32 {
            if idx < 0 {
                self.last[ch]
            } else {
                input[idx as usize * in_ch + ch]
            }
        };

        let mut out = Vec::with_capacity(((n as f64 / self.ratio) as usize + 2) * self.out_ch);
        let mut p = self.pos;
        while p < (n - 1) as f64 {
            let i = p.floor();
            let frac = (p - i) as f32;
            let i = i as isize;
            for c in 0..self.out_ch {
                let src = if in_ch == 1 { 0 } else { c }; // mono → duplica en todos los canales
                let a = sample(i, src);
                let b = sample(i + 1, src);
                out.push(a + (b - a) * frac);
            }
            p += self.ratio;
        }

        self.pos = p - n as f64;
        for ch in 0..in_ch {
            self.last[ch] = input[(n - 1) * in_ch + ch];
        }
        out
    }
}