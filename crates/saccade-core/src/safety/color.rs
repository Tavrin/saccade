//! sRGB, Machado (2009) colour-vision simulation, and CIEDE2000.

/// WCAG relative luminance, using IEC 61966-2-1 sRGB linearization.
pub fn linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts opaque sRGB bytes to linear RGB.
pub fn rgb(p: [u8; 3]) -> [f64; 3] {
    p.map(|v| linear(f64::from(v) / 255.0))
}

/// Relative luminance of linear RGB.
pub fn luminance(c: [f64; 3]) -> f64 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Machado, Oliveira & Fernandes, IEEE TVCG 15(6), 2009, supplementary
/// severity-1.0 matrices (doi:10.1109/TVCG.2009.113). Apply in linear RGB.
/// At severity 1.0 anomalous trichromacy reaches the dichromacy endpoint;
/// separate anomalous labels intentionally produce identical images.
pub const MACHADO: [[[f64; 3]; 3]; 3] = [
    [
        [0.152286, 1.052583, -0.204868],
        [0.114503, 0.786281, 0.099216],
        [-0.003882, -0.048116, 1.051998],
    ],
    [
        [0.367322, 0.860646, -0.227968],
        [0.280085, 0.672501, 0.047413],
        [-0.011820, 0.042940, 0.968881],
    ],
    [
        [1.255528, -0.076749, -0.178779],
        [-0.078411, 0.930809, 0.147602],
        [0.004733, 0.691367, 0.303900],
    ],
];

/// Simulates protan (0), deutan (1), or tritan (2), severity 1.0.
pub fn simulate(c: [f64; 3], kind: usize) -> [f64; 3] {
    MACHADO[kind.min(2)].map(|row| (row[0] * c[0] + row[1] * c[1] + row[2] * c[2]).clamp(0.0, 1.0))
}

/// Encodes linear RGB as sRGB bytes, clipping out-of-gamut values.
pub fn bytes(c: [f64; 3]) -> [u8; 3] {
    c.map(|v| (encode(v) * 255.0).round() as u8)
}

/// Linear sRGB to CIELAB, D65 white; CIE 1976 definition.
pub fn lab(c: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = c;
    let xyz = [
        (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047,
        0.2126729 * r + 0.7151522 * g + 0.0721750 * b,
        (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883,
    ];
    let f = xyz.map(|v| {
        if v > 216.0 / 24389.0 {
            v.cbrt()
        } else {
            (24389.0 / 27.0 * v + 16.0) / 116.0
        }
    });
    [
        116.0 * f[1] - 16.0,
        500.0 * (f[0] - f[1]),
        200.0 * (f[1] - f[2]),
    ]
}

/// CIEDE2000 with kL=kC=kH=1; Sharma, Wu & Dalal (2005), equations 1-22.
pub fn delta_e(a: [f64; 3], b: [f64; 3]) -> f64 {
    let [l1, a1, b1] = a;
    let [l2, a2, b2] = b;
    let cbar = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let g = 0.5 * (1.0 - (cbar.powi(7) / (cbar.powi(7) + 25f64.powi(7))).sqrt());
    let (ap1, ap2) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1, c2) = (ap1.hypot(b1), ap2.hypot(b2));
    let hue = |x: f64, y: f64| {
        if x == 0.0 && y == 0.0 {
            0.0
        } else {
            y.atan2(x).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1, h2) = (hue(ap1, b1), hue(ap2, b2));
    let mut dh = h2 - h1;
    if c1 * c2 == 0.0 {
        dh = 0.0;
    } else if dh > 180.0 {
        dh -= 360.0;
    } else if dh < -180.0 {
        dh += 360.0;
    }
    let d_h = 2.0 * (c1 * c2).sqrt() * (dh / 2.0).to_radians().sin();
    let (lm, cm) = ((l1 + l2) / 2.0, (c1 + c2) / 2.0);
    let hm = if c1 * c2 == 0.0 {
        h1 + h2
    } else if (h1 - h2).abs() <= 180.0 {
        (h1 + h2) / 2.0
    } else if h1 + h2 < 360.0 {
        (h1 + h2 + 360.0) / 2.0
    } else {
        (h1 + h2 - 360.0) / 2.0
    };
    let cos = |d: f64| d.to_radians().cos();
    let t = 1.0 - 0.17 * cos(hm - 30.0) + 0.24 * cos(2.0 * hm) + 0.32 * cos(3.0 * hm + 6.0)
        - 0.20 * cos(4.0 * hm - 63.0);
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cm;
    let sh = 1.0 + 0.015 * cm * t;
    let rt = -2.0
        * (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt()
        * (60.0 * (-((hm - 275.0) / 25.0).powi(2)).exp())
            .to_radians()
            .sin();
    let (dl, dc, dh) = ((l2 - l1) / sl, (c2 - c1) / sc, d_h / sh);
    (dl * dl + dc * dc + dh * dh + rt * dc * dh).max(0.0).sqrt()
}
