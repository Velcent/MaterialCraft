//! The static HTML report: a summary per subject, then per measured case Painter's map (left), ours (right),
//! the error heatmap and a slider that wipes between the two, with the numbers on top. Skipped cases are
//! listed with their reasons.

use super::images::CaseFiles;
use super::summary::{SubjectSummary, fmt_opt};
use crate::run::{CaseResult, Measured, Outcome};
use std::fmt::Write;

/// One case as the page shows it.
pub struct Entry {
    pub result: CaseResult,
    pub files: Option<CaseFiles>,
}

/// What the page header says about the run.
pub struct RunInfo<'a> {
    pub id: &'a str,
    pub time: &'a str,
    pub commit: &'a str,
    pub projects: usize,
}

pub fn page(info: &RunInfo, entries: &[Entry]) -> String {
    let results: Vec<CaseResult> = entries.iter().map(|e| e.result.clone()).collect();
    let mut h = String::with_capacity(64 * 1024);
    let _ = write!(
        h,
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">\
         <title>Parity {id}</title><style>{STYLE}</style></head><body><h1>Parity run {id}</h1>\
         <p class=meta>{time} · commit {commit} · {projects} project(s) · left: Painter, right: ours; numbers inside the UV \
         islands, MAE in 8-bit steps</p>",
        id = esc(info.id),
        time = esc(info.time),
        commit = esc(info.commit),
        projects = info.projects
    );
    summary(&mut h, &results);
    for s in SubjectSummary::of(&results) {
        let _ = write!(h, "<h2 id=\"{0}\">{0}</h2>", esc(s.subject));
        for e in entries.iter().filter(|e| e.result.subject == s.subject) {
            if let (Some(m), Some(f)) = (e.result.measured(), &e.files) {
                case(&mut h, &e.result, m, f);
            }
        }
        skipped(
            &mut h,
            entries.iter().map(|e| &e.result).filter(|r| r.subject == s.subject),
        );
    }
    h.push_str(SCRIPT);
    h.push_str("</body></html>\n");
    h
}

fn summary(h: &mut String, results: &[CaseResult]) {
    h.push_str(
        "<table><tr><th>subject<th>measured<th>skipped<th>failed<th>MAE mean<th>MAE median<th>≤2/255<th>SSIM<th>hist median<th>spec median</tr>",
    );
    for s in SubjectSummary::of(results) {
        let _ = write!(
            h,
            "<tr><td><a href=\"#{0}\">{0}</a><td>{1}<td>{2}<td>{3}<td>{4}<td>{5}<td>{6} %<td>{7}<td>{8}<td>{9}</tr>",
            esc(s.subject),
            s.measured,
            s.skipped,
            s.failed,
            fmt_opt(s.mae_mean, 2),
            fmt_opt(s.mae_median, 2),
            fmt_opt(s.within_mean, 1),
            fmt_opt(s.ssim_mean, 3),
            fmt_opt(s.histogram_median, 2),
            fmt_opt(s.spectrum_median, 3)
        );
    }
    h.push_str("</table>");
}

fn case(h: &mut String, r: &CaseResult, m: &Measured, f: &CaseFiles) {
    let d = &m.metrics.overall;
    let _ = write!(
        h,
        "<section class=case><h3>{label}</h3><p class=meta>{reference} ({rw}×{rh}) · compared at {w}×{h_} · high: {high} · \
         settings {hash} · {secs:.1} s · histogram distance {hist:.2} steps · spectrum distance {spec}</p><table><tr><th><th>MAE<th>max<th>PSNR<th>≤2/255<th>SSIM<th>fit gain / offset</tr>\
         <tr><td>all<td>{mae:.2}<td>{max:.0}<td>{psnr:.1}<td>{within:.1} %<td>{ssim}<td></tr>",
        label = esc(&r.label()),
        reference = esc(&m.reference),
        rw = m.reference_size.0,
        rh = m.reference_size.1,
        w = m.size.0,
        h_ = m.size.1,
        high = esc(&m.high),
        hash = esc(&m.settings_hash),
        secs = m.seconds,
        mae = m.metrics.mae_steps(),
        max = d.max * 255.0,
        psnr = d.psnr.min(99.9),
        within = d.within,
        ssim = fmt_opt(m.metrics.ssim, 3),
        hist = m.metrics.distribution.histogram,
        spec = fmt_opt(m.metrics.distribution.spectrum, 3),
    );
    for c in &m.metrics.components {
        let _ = write!(
            h,
            "<tr><td>{}<td>{:.2}<td>{:.0}<td>{:.1}<td>{:.1} %<td>{}<td>{:.3} / {:.3}</tr>",
            c.label,
            c.diff.mae * 255.0,
            c.diff.max * 255.0,
            c.diff.psnr.min(99.9),
            c.diff.within,
            fmt_opt(c.ssim, 3),
            c.fit.gain,
            c.fit.offset
        );
    }
    h.push_str("</table>");
    list(h, "notes", &m.notes);
    list(h, "Painter settings we do not reproduce", &m.unmapped);
    let _ = write!(
        h,
        "<div class=row><figure><img loading=lazy src=\"{t}\"><figcaption>Painter</figcaption></figure>\
         <figure><img loading=lazy src=\"{o}\"><figcaption>ours</figcaption></figure>\
         <figure><img loading=lazy src=\"{d}\"><figcaption>error (yellow ≥ 16/255)</figcaption></figure>\
         <figure class=wipe><div class=stack><img loading=lazy src=\"{o}\"><img class=over loading=lazy src=\"{t}\"></div>\
         <input type=range min=0 max=100 value=50 aria-label=\"wipe between Painter and ours\">\
         <figcaption>wipe: Painter ← → ours</figcaption></figure></div></section>",
        t = esc(&f.theirs),
        o = esc(&f.ours),
        d = esc(&f.diff),
    );
}

fn list(h: &mut String, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    let _ = write!(h, "<details><summary>{} ({})</summary><ul>", esc(title), items.len());
    items.iter().for_each(|i| {
        let _ = write!(h, "<li>{}", esc(i));
    });
    h.push_str("</ul></details>");
}

fn skipped<'a>(h: &mut String, results: impl Iterator<Item = &'a CaseResult>) {
    let rows: Vec<String> = results
        .filter_map(|r| match &r.outcome {
            Outcome::Skipped(why) => Some(format!("<tr><td>{}<td>skipped<td>{}</tr>", esc(&r.label()), esc(why))),
            Outcome::Failed(why) => Some(format!("<tr><td>{}<td>failed<td>{}</tr>", esc(&r.label()), esc(why))),
            Outcome::Measured(_) => None,
        })
        .collect();
    if !rows.is_empty() {
        let _ = write!(
            h,
            "<details><summary>not measured ({})</summary><table>{}</table></details>",
            rows.len(),
            rows.concat()
        );
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = ":root{--bg:#fafafa;--fg:#1d1d1f;--muted:#666;--line:#ddd;--card:#fff}\
@media (prefers-color-scheme:dark){:root{--bg:#141416;--fg:#e8e8ea;--muted:#9a9aa0;--line:#333;--card:#1e1e21}}\
body{margin:0 auto;padding:16px;max-width:1500px;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif}\
h1{font-size:22px}h2{margin-top:32px;border-bottom:1px solid var(--line)}h3{margin:0 0 4px;font-size:15px}\
.meta{color:var(--muted);margin:0 0 8px}table{border-collapse:collapse;margin:8px 0;font-variant-numeric:tabular-nums}\
td,th{padding:2px 10px;border-bottom:1px solid var(--line);text-align:right}td:first-child,th:first-child{text-align:left}\
.case{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:12px;margin:12px 0}\
.row{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:8px}figure{margin:0}\
figure img{width:100%;display:block;image-rendering:pixelated;background:#000}figcaption{color:var(--muted);font-size:12px}\
.stack{position:relative}.stack .over{position:absolute;inset:0;clip-path:inset(0 50% 0 0)}input[type=range]{width:100%}\
details{margin:4px 0}a{color:inherit}";

const SCRIPT: &str = "<script>document.querySelectorAll('.wipe').forEach(f=>{const r=f.querySelector('input'),o=f.querySelector('.over');\
r.addEventListener('input',()=>{o.style.clipPath='inset(0 '+(100-r.value)+'% 0 0)'})});</script>";

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn escapes_and_lists_skips() {
        assert_eq!(esc("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
        let entry = Entry {
            result: CaseResult {
                subject: "baked.ao",
                project: PathBuf::from("x.spp"),
                project_name: "x.spp".into(),
                set: "<set>".into(),
                outcome: Outcome::Skipped("no high-poly".into()),
            },
            files: None,
        };
        let info = RunInfo {
            id: "r",
            time: "t",
            commit: "c",
            projects: 1,
        };
        let html = page(&info, &[entry]);
        assert!(html.contains("x.spp / &lt;set&gt;") && html.contains("no high-poly"));
        assert!(html.starts_with("<!doctype html>"));
    }
}
