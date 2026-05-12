use std::fs;

const GUTTER_L: u32 = 60;
const GUTTER_R: u32 = 70;
const GUTTER_T: u32 = 30;
const GUTTER_B: u32 = 50;

pub fn write_heatmap_html(
    png_basename: &str,
    wav_basename: &str,
    audio_duration_s: f64,
    img_width: u32,
    img_height: u32,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let png_safe = escape_attr(png_basename);
    let wav_safe = escape_attr(wav_basename);

    let total_w = img_width + GUTTER_L + GUTTER_R;
    let total_h = img_height + GUTTER_T + GUTTER_B;
    let img_bottom = GUTTER_T + img_height;

    let html = HTML_TEMPLATE
        .replace("{{PNG_BASENAME}}", &png_safe)
        .replace("{{WAV_BASENAME}}", &wav_safe)
        .replace("{{TITLE}}", wav_basename)
        .replace("{{TOTAL_WIDTH}}", &total_w.to_string())
        .replace("{{TOTAL_HEIGHT}}", &total_h.to_string())
        .replace("{{IMG_WIDTH}}", &img_width.to_string())
        .replace("{{IMG_HEIGHT}}", &img_height.to_string())
        .replace("{{IMG_BOTTOM}}", &img_bottom.to_string())
        .replace("{{GUTTER_L}}", &GUTTER_L.to_string())
        .replace("{{GUTTER_T}}", &GUTTER_T.to_string())
        .replace("{{GUTTER_B}}", &GUTTER_B.to_string())
        .replace("{{GUTTER_R}}", &GUTTER_R.to_string())
        .replace("{{AUDIO_DURATION_S}}", &format!("{:.3}", audio_duration_s));

    fs::write(path, html)?;
    Ok(())
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Cicada — {{TITLE}}</title>
<style>
  body {
    margin: 0;
    background: #111;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 16px;
    font-family: monospace;
    color: #ccc;
  }
  h1 { font-size: 13px; margin: 0 0 8px 0; color: #777; letter-spacing: 0.05em; }
  #stage { width: 100%; max-width: {{TOTAL_WIDTH}}px; }
  svg { display: block; width: 100%; }
  audio {
    margin-top: 10px;
    width: 100%;
    max-width: {{TOTAL_WIDTH}}px;
    accent-color: #e07040;
  }
</style>
</head>
<body>
<h1>{{TITLE}}</h1>
<div id="stage">
  <svg id="viz"
       viewBox="0 0 {{TOTAL_WIDTH}} {{TOTAL_HEIGHT}}"
       xmlns="http://www.w3.org/2000/svg">

    <!-- Heatmap image -->
    <image href="{{PNG_BASENAME}}"
           x="{{GUTTER_L}}" y="{{GUTTER_T}}"
           width="{{IMG_WIDTH}}" height="{{IMG_HEIGHT}}"
           preserveAspectRatio="none"/>

    <!-- Axes and ticks (filled by JS) -->
    <g id="axes"></g>

    <!-- Playback pointer — updated by rAF loop -->
    <line id="pointer"
          x1="{{GUTTER_L}}" x2="{{GUTTER_L}}"
          y1="{{GUTTER_T}}" y2="{{IMG_BOTTOM}}"
          stroke="white" stroke-width="1.5" opacity="0.75"
          pointer-events="none"/>
  </svg>
</div>
<audio id="player" src="{{WAV_BASENAME}}" controls></audio>

<script>
// ── Constants injected by Cicada ────────────────────────────────────────────
const CFG = {
  imgW:     {{IMG_WIDTH}},
  imgH:     {{IMG_HEIGHT}},
  duration: {{AUDIO_DURATION_S}},
  gL: {{GUTTER_L}},
  gT: {{GUTTER_T}},
  gB: {{GUTTER_B}},
  gR: {{GUTTER_R}},
};

// ── m/z ↔ frequency helpers (mirrors oscillator.rs mz_to_freq) ─────────────
const MZ_MIN = 300, MZ_MAX = 1000;
const HZ_MIN = 30,  HZ_MAX = 4200;

function mzToHz(mz) {
  const t = (mz - MZ_MIN) / (MZ_MAX - MZ_MIN);
  return Math.round(Math.exp(Math.log(HZ_MIN) + t * Math.log(HZ_MAX / HZ_MIN)));
}

function mzToSvgY(mz) {
  const t = (mz - MZ_MIN) / (MZ_MAX - MZ_MIN);
  return CFG.gT + (1.0 - t) * (CFG.imgH - 1);
}

function timeToSvgX(t) {
  if (CFG.duration <= 0) return CFG.gL;
  return CFG.gL + (t / CFG.duration) * (CFG.imgW - 1);
}

// ── SVG helper ───────────────────────────────────────────────────────────────
const NS = 'http://www.w3.org/2000/svg';
function svgEl(tag, attrs) {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  return el;
}
function addLine(g, x1, y1, x2, y2, stroke, sw) {
  g.appendChild(svgEl('line', { x1, y1, x2, y2, stroke, 'stroke-width': sw }));
}
function addText(g, x, y, txt, anchor, fill, fontSize) {
  const el = svgEl('text', {
    x, y,
    'text-anchor': anchor,
    fill: fill || '#aaa',
    'font-size': fontSize || 10,
    'font-family': 'monospace',
  });
  el.textContent = txt;
  g.appendChild(el);
}

// ── Axis drawing (runs once) ─────────────────────────────────────────────────
function drawAxes() {
  const g = document.getElementById('axes');
  const x0 = CFG.gL, x1 = CFG.gL + CFG.imgW;
  const y0 = CFG.gT, y1 = CFG.gT + CFG.imgH;

  // Image border
  addLine(g, x0, y0, x1, y0, '#555', 0.5);
  addLine(g, x0, y1, x1, y1, '#555', 0.5);
  addLine(g, x0, y0, x0, y1, '#555', 0.5);
  addLine(g, x1, y0, x1, y1, '#555', 0.5);

  // Y axis — m/z (left) and Hz (right)
  for (let mz = MZ_MIN; mz <= MZ_MAX; mz += 100) {
    const y = mzToSvgY(mz);
    // Left: m/z tick and label
    addLine(g, x0 - 5, y, x0, y, '#666', 0.8);
    addText(g, x0 - 7, y + 3.5, mz, 'end', '#999');
    // Right: Hz tick and label
    const hz = mzToHz(mz);
    addLine(g, x1, y, x1 + 5, y, '#666', 0.8);
    addText(g, x1 + 7, y + 3.5, hz + ' Hz', 'start', '#5a9aba');
  }

  // Y axis labels
  addText(g, x0 - 45, y0 - 10, 'm/z', 'start', '#777', 11);
  addText(g, x1 + 5,  y0 - 10, 'Hz',  'start', '#5a9aba', 11);

  // X axis — time ticks
  if (CFG.duration > 0) {
    const totalW = CFG.gL + CFG.imgW + CFG.gR;
    const targetTicks = 10;
    // Pick a "nice" interval
    const raw = CFG.duration / targetTicks;
    const magnitude = Math.pow(10, Math.floor(Math.log10(raw)));
    const normalized = raw / magnitude;
    let nice;
    if (normalized < 1.5) nice = 1;
    else if (normalized < 3.5) nice = 2;
    else if (normalized < 7.5) nice = 5;
    else nice = 10;
    const interval = nice * magnitude;

    for (let t = 0; t <= CFG.duration + interval * 0.01; t += interval) {
      const x = timeToSvgX(Math.min(t, CFG.duration));
      addLine(g, x, y1, x, y1 + 5, '#666', 0.8);
      const label = t >= 60 ? (t / 60).toFixed(1) + ' min' : t.toFixed(1) + ' s';
      addText(g, x, y1 + 16, label, 'middle', '#999');
    }

    addText(g, totalW / 2, y1 + CFG.gB - 4, 'Time', 'middle', '#777', 11);
  }
}

// ── Playback pointer (rAF loop) ───────────────────────────────────────────────
window.addEventListener('DOMContentLoaded', function () {
  drawAxes();

  const player  = document.getElementById('player');
  const pointer = document.getElementById('pointer');

  function tick() {
    const x = timeToSvgX(player.currentTime);
    pointer.setAttribute('x1', x);
    pointer.setAttribute('x2', x);
    requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
});
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_attr() {
        assert_eq!(escape_attr("file\"name"), "file&quot;name");
        assert_eq!(escape_attr("a<b>c"), "a&lt;b&gt;c");
        assert_eq!(escape_attr("normal"), "normal");
    }

    #[test]
    fn test_write_heatmap_html_placeholders() {
        let tmp = std::env::temp_dir().join("cicada_test_viewer.html");
        let result = write_heatmap_html(
            "out_ms1_heatmap.png",
            "out_ms1.wav",
            120.0,
            1600,
            800,
            tmp.to_str().unwrap(),
        );
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&tmp).unwrap();
        // No unreplaced placeholders should remain
        assert!(!content.contains("{{"), "unreplaced placeholder found");
        // Key content present
        assert!(content.contains("out_ms1_heatmap.png"));
        assert!(content.contains("out_ms1.wav"));
        assert!(content.contains("120.000"));
    }
}
