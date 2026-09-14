use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn write_page(output_dir: &Path, names: &[&str], img_prefix: &str) -> Result<std::path::PathBuf> {
    fs::create_dir_all(output_dir)?;
    let js_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../visual-review/review.js");
    let js_dest = output_dir.join("review.js");
    if js_src.canonicalize().ok() != js_dest.canonicalize().ok() {
        fs::copy(&js_src, &js_dest)?;
    }
    let mut cards = String::new();
    for name in names {
        cards.push_str(&format!(
            "<label class=\"card\"><input type=\"checkbox\" data-name=\"{name}\">\
             <img src=\"{img_prefix}{name}.png\" alt=\"{name}\"><span>{name}</span></label>\n"
        ));
    }
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>Xenon visual review</title>
<style>
  :root {{ color-scheme: dark; --bg:#0b0b0c; --fg:#e8e8ea; --muted:#8a8a90; --border:#2a2a2e; --accent:#6c9eff; }}
  * {{ box-sizing: border-box; }}
  body {{ margin:0; font-family: system-ui, sans-serif; background:var(--bg); color:var(--fg); }}
  header {{ position:sticky; top:0; z-index:1; display:flex; gap:12px; align-items:center; flex-wrap:wrap;
            padding:12px 16px; border-bottom:1px solid var(--border); background:#141416; }}
  h1 {{ font-size:16px; margin:0; font-weight:600; }}
  button {{ background:var(--accent); color:#000; border:0; border-radius:8px; padding:8px 12px; font-weight:600; }}
  #copy-out {{ flex:1; min-width:240px; font: 13px ui-monospace, menlo, monospace; padding:8px;
               border-radius:8px; border:1px solid var(--border); background:#0b0b0c; color:var(--fg); }}
  .grid {{ display:grid; grid-template-columns:repeat(auto-fill, minmax(320px, 1fr)); gap:16px; padding:16px; }}
  .card {{ display:flex; flex-direction:column; gap:8px; border:1px solid var(--border); border-radius:12px;
           padding:10px; background:#141416; }}
  .card img {{ width:100%; height:auto; border-radius:8px; background:#000; }}
  .card span {{ font-size:12px; color:var(--muted); }}
  .card:has(input:checked) {{ outline:2px solid var(--accent); }}
</style>
</head>
<body>
<header>
  <h1>Xenon screenshots</h1>
  <button type="button" id="copy">Copy disapproved</button>
  <textarea id="copy-out" rows="3" placeholder="Select shots, then copy a pasteable list."></textarea>
</header>
<main class="grid">
{cards}
</main>
<script src="review.js"></script>
<script>
(function () {{
  function items() {{
    var nodes = document.querySelectorAll('input[type=checkbox][data-name]');
    var out = [];
    for (var i = 0; i < nodes.length; i++) {{
      out.push({{ name: nodes[i].getAttribute('data-name'), checked: nodes[i].checked }});
    }}
    return out;
  }}
  function refresh() {{
    document.getElementById('copy-out').value = disapprovalClipboard(selectedNames(items()));
  }}
  document.querySelector('main').addEventListener('change', refresh);
  document.getElementById('copy').addEventListener('click', function () {{
    refresh();
    var box = document.getElementById('copy-out');
    box.focus();
    box.select();
    if (navigator.clipboard && navigator.clipboard.writeText) {{
      navigator.clipboard.writeText(box.value);
    }}
  }});
}})();
</script>
</body>
</html>
"#
    );
    let path = output_dir.join("index.html");
    fs::write(&path, html)?;
    Ok(path)
}
