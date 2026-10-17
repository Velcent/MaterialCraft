// ======================================================================= UI (included into gui.rs)

struct NodeSnap {
    id: u32,
    kind: String,
    slots: Vec<(&'static str, bool)>,
    ins: Vec<(String, u32)>,
    pos: [f32; 2],
    graph_name: Option<String>,
    dot: bool,
    /// Output names of a graph node with several outputs (empty = a single default output).
    outs: Vec<String>,
    in_out: Vec<(String, String)>,
}

impl App {
    fn screen_to_world(&self, p: Pos2) -> Pos2 {
        let w = ((p - self.canvas_rect.min) - self.pan) / self.zoom;
        pos2(w.x, w.y)
    }

    fn open_search(&mut self, screen: Pos2, link_from: Option<u32>) {
        let w = self.screen_to_world(screen);
        self.search = Some(SearchState { screen, world: pos2(w.x - CARD * 0.5, w.y - CARD_H * 0.5), text: String::new(), idx: 0, link_from, link_to: None, fresh: true });
    }

    fn open_search_to(&mut self, screen: Pos2, dst: u32, slot: String) {
        self.open_search(screen, None);
        if let Some(st) = self.search.as_mut() {
            st.link_to = Some((dst, slot));
        }
    }

    // ------------------------------------------------------------ graph canvas
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.canvas_rect = rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(58, 58, 58));
        let mods = ui.input(|i| i.modifiers);
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let released = ui.input(|i| i.pointer.any_released());
        let mut acts: Vec<Act> = Vec::new();
        // taking a payload of the wrong type would discard it, so check which kind is being dragged first
        if egui::DragAndDrop::has_payload_of_type::<GraphDrag>(ui.ctx()) {
            if let (Some(payload), Some(p)) = (resp.dnd_release_payload::<GraphDrag>(), pointer) {
                acts.push(Act::ImportFromPkg(payload.0, payload.2.clone(), Some(p)));
            }
        } else if let (Some(payload), Some(p)) = (resp.dnd_release_payload::<String>(), pointer) {
            acts.push(Act::Drop((*payload).clone(), p));
        }

        if resp.hovered() {
            let sc = ui.input(|i| i.raw_scroll_delta.y);
            if sc != 0.0 {
                let old = self.zoom;
                let nz = (old * (1.0 + sc * 0.0012)).clamp(0.25, 2.5);
                if let Some(p) = pointer {
                    let wp = ((p - rect.min) - self.pan) / old;
                    self.pan = (p - rect.min) - wp * nz;
                }
                self.zoom = nz;
            }
        }
        let pan_mode = mods.alt || ui.input(|i| i.key_down(Key::Space));
        if resp.dragged_by(PointerButton::Middle) || (resp.dragged_by(PointerButton::Primary) && pan_mode && (self.connecting.is_none() && self.connecting_in.is_none())) {
            self.pan += resp.drag_delta();
        }
        if resp.drag_started_by(PointerButton::Primary) && !pan_mode && (self.connecting.is_none() && self.connecting_in.is_none()) {
            self.rubber = pointer;
            if !mods.shift {
                self.sel.clear();
                self.sel_comment = None;
            }
        }
        if resp.clicked() && !mods.shift {
            self.sel.clear();
            self.sel_comment = None;
        }

        let z = self.zoom;
        let org = rect.min + self.pan;
        let ts = move |p: [f32; 2]| pos2(org.x + p[0] * z, org.y + p[1] * z);

        // grid
        let minor = Stroke::new(1.0_f32, Color32::from_rgb(63, 63, 63));
        let major = Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 50));
        let step = 32.0 * z;
        if step > 6.0 {
            let (ox, oy) = (self.pan.x.rem_euclid(step), self.pan.y.rem_euclid(step));
            let mut x = rect.min.x + ox;
            let mut k = ((rect.min.x + ox - org.x) / step).round() as i64;
            while x < rect.max.x {
                painter.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], if k.rem_euclid(4) == 0 { major } else { minor });
                x += step;
                k += 1;
            }
            let mut y = rect.min.y + oy;
            let mut k = ((rect.min.y + oy - org.y) / step).round() as i64;
            while y < rect.max.y {
                painter.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], if k.rem_euclid(4) == 0 { major } else { minor });
                y += step;
                k += 1;
            }
        }

        // comment frames (drawn under nodes)
        for ci in 0..self.g.comments.len() {
            let c = self.g.comments[ci].clone();
            let r = Rect::from_min_size(ts(c.pos), vec2(c.size[0], c.size[1]) * z);
            let col = COMMENT_COLORS[c.color as usize % COMMENT_COLORS.len()];
            let base = Color32::from_rgb(col[0], col[1], col[2]);
            let selected = self.sel_comment == Some(ci);
            painter.rect_filled(r, Rounding::same(4.0 * z), base.gamma_multiply(0.16));
            let title = Rect::from_min_size(r.min, vec2(r.width(), 22.0 * z));
            painter.rect_filled(title, Rounding { nw: 4.0 * z, ne: 4.0 * z, sw: 0.0, se: 0.0 }, base.gamma_multiply(0.5));
            painter.rect_stroke(r, Rounding::same(4.0 * z), Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, if selected { Color32::from_rgb(255, 160, 40) } else { base.gamma_multiply(0.8) }));
            painter.text(title.left_center() + vec2(8.0 * z, 0.0), Align2::LEFT_CENTER, &c.text, FontId::proportional(13.0 * z), Color32::WHITE);
            let tr = ui.interact(title, Id::new(("cmt", ci)), Sense::click_and_drag());
            if tr.drag_started() {
                self.snap();
                let cr = Rect::from_min_size(pos2(c.pos[0], c.pos[1]), vec2(c.size[0], c.size[1]));
                self.comment_members = self.g.nodes.iter().filter(|(_, n)| cr.contains(pos2(n.pos[0] + CARD * 0.5, n.pos[1] + CARD_H * 0.5))).map(|(i, _)| *i).collect();
                self.sel_comment = Some(ci);
                self.sel.clear();
            }
            if tr.clicked() {
                self.sel_comment = Some(ci);
                self.sel.clear();
            }
            if tr.dragged() {
                let d = tr.drag_delta() / z;
                self.g.comments[ci].pos[0] += d.x;
                self.g.comments[ci].pos[1] += d.y;
                for id in self.comment_members.clone() {
                    if let Some(n) = self.g.nodes.get_mut(&id) {
                        n.pos[0] += d.x;
                        n.pos[1] += d.y;
                    }
                }
            }
            let hs = vec2(14.0, 14.0) * z;
            let handle = Rect::from_min_size(r.max - hs, hs);
            painter.rect_filled(handle, Rounding::same(2.0), base.gamma_multiply(0.7));
            let hr = ui.interact(handle, Id::new(("cmtr", ci)), Sense::drag());
            if hr.drag_started() {
                self.snap();
            }
            if hr.dragged() {
                let d = hr.drag_delta() / z;
                self.g.comments[ci].size[0] = (self.g.comments[ci].size[0] + d.x).max(80.0);
                self.g.comments[ci].size[1] = (self.g.comments[ci].size[1] + d.y).max(50.0);
            }
        }

        // snapshot of nodes for this frame
        let snaps: Vec<NodeSnap> = self
            .g
            .nodes
            .iter()
            .map(|(id, n)| NodeSnap {
                id: *id,
                kind: n.kind.clone(),
                // a graph node only has input sockets when that graph defines inputs
                slots: if n.kind == "subgraph" && !n.strs.get("graph").and_then(|g| self.g.subgraphs.get(g)).map(|sg| sg.nodes.values().any(|x| x.kind == "input")).unwrap_or(true) { Vec::new() } else { slots(&n.kind) },
                ins: n.ins.iter().map(|(k, v)| (k.clone(), *v)).collect(),
                pos: n.pos,
                graph_name: n.strs.get("graph").cloned(),
                dot: n.kind == "dot",
                outs: n.strs.get("graph").filter(|_| n.kind == "subgraph").and_then(|g| self.g.subgraphs.get(g)).map(|sg| sg.output_idents()).filter(|o| o.len() >= 2).unwrap_or_default(),
                in_out: n.in_out.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            })
            .collect();
        let body_of = |s: &NodeSnap| card_body(s.slots.len(), s.outs.len());
        let posmap: HashMap<u32, ([f32; 2], bool, f32)> = snaps.iter().map(|s| (s.id, (s.pos, s.dot, body_of(s)))).collect();
        let out_port = move |p: [f32; 2], dot: bool, body: f32| if dot { ts(p) + vec2(24.0 * z, 12.0 * z) } else { ts(p) + vec2(CARD * z, (BAR + body * 0.5) * z) };
        let outs_map: HashMap<u32, Vec<String>> = snaps.iter().filter(|s| !s.outs.is_empty()).map(|s| (s.id, s.outs.clone())).collect();
        let out_port_i = move |p: [f32; 2], i: usize, n: usize, body: f32| ts(p) + vec2(CARD * z, (BAR + body * (i as f32 + 1.0) / (n as f32 + 1.0)) * z);
        // where the link of output `name` of node `src` leaves its card
        let src_port = |src: u32, name: Option<&str>| -> Option<Pos2> {
            let sp = posmap.get(&src)?;
            Some(match outs_map.get(&src) {
                Some(o) => out_port_i(sp.0, name.and_then(|nm| o.iter().position(|x| x == nm)).unwrap_or(0), o.len(), sp.2),
                None => out_port(sp.0, sp.1, sp.2),
            })
        };
        let in_port = move |p: [f32; 2], i: usize, n: usize, dot: bool, body: f32| if dot { ts(p) + vec2(0.0, 12.0 * z) } else { ts(p) + vec2(0.0, (BAR + body * (i as f32 + 1.0) / (n as f32 + 1.0)) * z) };
        let link = |painter: &egui::Painter, a: Pos2, b: Pos2, col: Color32, w: f32| {
            let dx = ((b.x - a.x).abs() * 0.5).max(40.0 * z);
            painter.add(CubicBezierShape::from_points_stroke([a, a + vec2(dx, 0.0), b - vec2(dx, 0.0), b], false, Color32::TRANSPARENT, Stroke::new(w * z.max(0.7), col)));
        };

        // links
        for s in &snaps {
            let n = s.slots.len();
            for (slot, src) in &s.ins {
                if let (Some(i), Some(a)) = (s.slots.iter().position(|x| x.0 == slot), src_port(*src, s.in_out.iter().find(|(k, _)| k == slot).map(|(_, v)| v.as_str()))) {
                    let hot = self.sel.contains(&s.id) || self.sel.contains(src);
                    link(&painter, a, in_port(s.pos, i, n, s.dot, body_of(s)), if hot { Color32::from_rgb(255, 170, 60) } else { Color32::from_rgb(185, 185, 185) }, 1.8);
                }
            }
        }

        let out_names: Vec<&'static str> = OUTPUT_NAMES.to_vec();
        let mut ports: Vec<(u32, &'static str, Pos2)> = Vec::new();
        let mut out_ports: Vec<(u32, Option<String>, Pos2)> = Vec::new();
        // sockets that have a link attached (drawn filled orange)
        let used_ports: HashSet<(u32, String)> = snaps
            .iter()
            .flat_map(|s| s.ins.iter().map(move |(slot, src)| (*src, s.in_out.iter().find(|(k, _)| k == slot).map(|(_, v)| v.clone()))))
            .filter_map(|(src, name)| outs_map.get(&src).map(|o| (src, name.unwrap_or_else(|| o[0].clone()))))
            .collect();
        let mut cards: Vec<Rect> = Vec::new();

        for s in &snaps {
            let id = s.id;
            let card = Rect::from_min_size(ts(s.pos), if s.dot { vec2(24.0, 24.0) } else { vec2(CARD, BAR + body_of(s)) } * z);
            cards.push(card);
            let selected = self.sel.contains(&id);
            let err = self.node_err.get(&id).cloned();
            if s.dot {
                let sel_col = if selected { Color32::from_rgb(255, 160, 40) } else { Color32::from_gray(150) };
                painter.circle(card.center(), 9.0 * z, Color32::from_rgb(38, 38, 38), Stroke::new(if selected { 2.4_f32 } else { 1.3_f32 }, sel_col));
                painter.circle_filled(card.center(), 3.2 * z, Color32::from_gray(200));
            } else {
            painter.rect_filled(card.translate(vec2(2.0, 3.0) * z), Rounding::same(3.0 * z), Color32::from_black_alpha(70));
            painter.rect_filled(card, Rounding::same(3.0 * z), Color32::from_rgb(32, 32, 32));
            let bar = Rect::from_min_size(card.min, vec2(card.width(), BAR * z));
            painter.rect_filled(bar, Rounding { nw: 3.0 * z, ne: 3.0 * z, sw: 0.0, se: 0.0 }, kind_color(&s.kind));
            let node_ref = self.g.nodes.get(&id);
            let usage = node_ref.and_then(|n| n.strs.get("usage").cloned()).unwrap_or_else(|| "basecolor".into());
            let ident = node_ref.and_then(|n| n.strs.get("identifier").cloned()).filter(|x| !x.is_empty()).unwrap_or_else(|| usage.clone());
            let label = match (&s.graph_name, s.kind.as_str()) {
                (Some(g), "subgraph") => g.clone(),
                (_, "output") => usage_label(&usage),
                _ => label(&s.kind),
            };
            let fs = if label.len() > 16 { 8.0 } else if label.len() > 12 { 9.0 } else { 10.5 };
            painter.text(bar.center(), Align2::CENTER_CENTER, label, FontId::proportional(fs * z), Color32::WHITE);
            let th = Rect::from_min_size(card.min + vec2(3.0, BAR + 3.0 + (body_of(s) - CARD) * 0.5) * z, vec2(CARD - 6.0, CARD - 6.0) * z);
            match self.thumbs.get(&id) {
                Some(t) => {
                    painter.image(t.id(), th, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => {
                    painter.rect_filled(th, 0.0, Color32::from_rgb(22, 22, 22));
                }
            }
            if err.is_some() {
                painter.rect_filled(th, 0.0, Color32::from_rgba_unmultiplied(170, 30, 30, 150));
                painter.text(th.center(), Align2::CENTER_CENTER, "!", FontId::proportional(30.0 * z), Color32::WHITE);
            }
            painter.rect_stroke(card, Rounding::same(3.0 * z), Stroke::new(if selected { 2.4_f32 } else { 1.0_f32 }, if selected { Color32::from_rgb(255, 160, 40) } else { Color32::from_rgb(14, 14, 14) }));
            if self.active == Some(id) && !selected {
                painter.rect_stroke(card, Rounding::same(3.0 * z), Stroke::new(1.5_f32, Color32::from_rgb(150, 150, 150)));
            }

            if s.kind == "output" {
                let bc = card.left_top() + vec2(3.0, -1.0) * z;
                painter.circle(bc, 10.5 * z, Color32::from_rgb(30, 30, 30), Stroke::new(1.5_f32, Color32::from_gray(205)));
                painter.circle_stroke(bc, 5.6 * z, Stroke::new(1.3_f32, Color32::from_gray(235)));
                let mut pts = Vec::new();
                for k in 0..=24 {
                    let a = k as f32 / 24.0 * std::f32::consts::TAU;
                    pts.push(bc + vec2(a.cos() * 5.6, a.sin() * 2.2) * z);
                }
                painter.add(egui::Shape::line(pts, Stroke::new(1.0_f32, Color32::from_gray(200))));
                painter.circle_filled(bc + vec2(-1.8, -1.8) * z, 1.4 * z, Color32::WHITE);
                painter.text(card.left_top() + vec2(16.0, -4.0) * z, Align2::LEFT_BOTTOM, format!("({ident})"), FontId::proportional(9.0 * z), Color32::from_gray(150));
                let bits = if self.g.meta.get("output_format").map(|f| f == "16").unwrap_or(false) { 16 } else { 8 };
                painter.text(card.center_bottom() + vec2(0.0, 3.0) * z, Align2::CENTER_TOP, format!("{0}x{0} - C{1}", self.nominal_node_size(id), bits), FontId::proportional(10.5 * z), Color32::from_gray(185));
            }
            }

            // a node with its own Output Size shows it under the card, like Designer
            if s.kind != "output" && !matches!(s.kind.as_str(), "comment" | "frame" | "dot") {
                let ns = self.nominal_node_size(id);
                if ns != self.g.size {
                    painter.text(card.center_bottom() + vec2(0.0, 3.0) * z, Align2::CENTER_TOP, format!("{0}x{0}", ns), FontId::proportional(10.0 * z), Color32::from_rgb(255, 190, 110));
                }
            }

            // interaction: card
            let nr = ui.interact(card, Id::new(("node", id)), Sense::click_and_drag());
            if nr.drag_started() {
                self.snap();
                if !self.sel.contains(&id) {
                    if !(mods.shift || mods.command) {
                        self.sel.clear();
                    }
                    self.sel.insert(id);
                }
                self.sel_comment = None;
                if self.active != Some(id) {
                    self.active = Some(id);
                    self.dirty = true;
                }
            }
            if nr.dragged() && (self.connecting.is_none() && self.connecting_in.is_none()) {
                let d = nr.drag_delta() / z;
                for sid in self.sel.clone() {
                    if let Some(n) = self.g.nodes.get_mut(&sid) {
                        n.pos[0] += d.x;
                        n.pos[1] += d.y;
                    }
                }
            }
            if nr.clicked() {
                if mods.shift || mods.command {
                    if !self.sel.remove(&id) {
                        self.sel.insert(id);
                    }
                } else {
                    self.sel = [id].into_iter().collect();
                }
                self.sel_comment = None;
                if self.active != Some(id) {
                    self.active = Some(id);
                    self.dirty = true;
                }
            }
            if nr.double_clicked() {
                if let (Some(g), "subgraph") = (&s.graph_name, s.kind.as_str()) {
                    acts.push(Act::EnterSub(g.clone()));
                }
            }
            if nr.secondary_clicked() && !self.sel.contains(&id) {
                self.sel = [id].into_iter().collect();
                self.active = Some(id);
                self.dirty = true;
            }
            if let Some(e) = &err {
                nr.clone().on_hover_text(e);
            }
            let (kind, gname) = (s.kind.clone(), s.graph_name.clone());
            nr.context_menu(|ui| {
                if ui.button("Copy").clicked() {
                    acts.push(Act::Copy);
                    ui.close_menu();
                }
                if ui.button("Duplicate").clicked() {
                    acts.push(Act::Duplicate);
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    acts.push(Act::Delete);
                    ui.close_menu();
                }
                ui.separator();
                if kind == "subgraph" {
                    if let Some(g) = &gname {
                        if ui.button("Open subgraph").clicked() {
                            acts.push(Act::EnterSub(g.clone()));
                            ui.close_menu();
                        }
                    }
                }
                if ui.button("Create subgraph from selection").clicked() {
                    acts.push(Act::Subgraph);
                    ui.close_menu();
                }
                ui.menu_button("Add output node", |ui| {
                    for name in &out_names {
                        if ui.button(usage_label(name)).clicked() {
                            acts.push(Act::NewOutput(name.to_string(), id));
                            ui.close_menu();
                        }
                    }
                });
            });

            // ports
            if s.kind != "output" && s.outs.len() >= 2 {
                // a graph node: one socket per output, labelled to the right of the card
                for (oi, oname) in s.outs.iter().enumerate() {
                    let op = out_port_i(s.pos, oi, s.outs.len(), body_of(s));
                    out_ports.push((id, Some(oname.clone()), op));
                    let used = used_ports.contains(&(id, oname.clone()));
                    painter.circle(op, 4.8 * z, if used { Color32::from_rgb(240, 170, 50) } else { Color32::from_rgb(200, 200, 200) }, Stroke::new(1.0_f32, Color32::BLACK));
                    if z >= 0.45 {
                        painter.text(op + vec2(9.0 * z, 0.0), Align2::LEFT_CENTER, usage_label(oname), FontId::proportional(10.5 * z.max(0.7)), Color32::from_gray(175));
                    }
                    let pr = ui.interact(Rect::from_center_size(op, Vec2::splat(14.0 * z)), Id::new(("out", id, oi)), Sense::drag());
                    if pr.drag_started() {
                        self.connecting = Some(id);
                        self.connecting_out = Some(oname.clone());
                    }
                }
            } else if s.kind != "output" {
                let op = out_port(s.pos, s.dot, body_of(s));
                out_ports.push((id, None, op));
                painter.circle(op, if s.dot { 3.5 } else { 4.8 } * z, Color32::from_rgb(235, 235, 235), Stroke::new(1.0_f32, Color32::BLACK));
                let pr = ui.interact(Rect::from_center_size(op, Vec2::splat(14.0 * z)), Id::new(("out", id)), Sense::drag());
                if pr.drag_started() {
                    self.connecting = Some(id);
                    self.connecting_out = None;
                }
            }
            let n = s.slots.len();
            for (i, (slot, required)) in s.slots.iter().enumerate() {
                let ip = in_port(s.pos, i, n, s.dot, body_of(s));
                ports.push((id, *slot, ip));
                painter.circle(ip, 4.5 * z, if s.kind == "output" { Color32::from_rgb(240, 170, 50) } else if *required { Color32::from_rgb(210, 210, 210) } else { Color32::from_rgb(60, 60, 60) }, Stroke::new(1.2_f32, Color32::from_rgb(190, 190, 190)));
                let ir = ui.interact(Rect::from_center_size(ip, Vec2::splat(14.0 * z)), Id::new(("in", id, i)), Sense::drag());
                if ir.drag_started() {
                    let had = self.g.nodes.get(&id).and_then(|nd| nd.ins.get(*slot).copied());
                    if let Some(old) = had {
                        self.snap();
                        let mut old_out = None;
                        if let Some(nd) = self.g.nodes.get_mut(&id) {
                            nd.ins.remove(*slot);
                            old_out = nd.in_out.remove(*slot);
                        }
                        self.connecting = Some(old);
                        self.connecting_out = old_out;
                        self.dirty = true;
                    } else {
                        self.connecting_in = Some((id, slot.to_string()));
                    }
                }
                if ir.hovered() {
                    painter.text(ip + vec2(-8.0 * z, 0.0), Align2::RIGHT_CENTER, *slot, FontId::proportional(10.5 * z.max(0.8)), Color32::WHITE);
                }
            }
        }

        // connection drag
        if let Some(src) = self.connecting {
            if let (Some(pt), Some(a)) = (pointer, src_port(src, self.connecting_out.as_deref())) {
                link(&painter, a, pt, Color32::from_rgb(255, 190, 70), 2.0);
            }
            if released {
                if let Some(pt) = pointer {
                    let near = ports.iter().filter(|(_, _, p)| (*p - pt).length() < 14.0 * z).min_by(|a, b| (a.2 - pt).length().partial_cmp(&(b.2 - pt).length()).unwrap_or(std::cmp::Ordering::Equal));
                    if let Some((dst, slot, _)) = near {
                        let (dst, slot) = (*dst, slot.to_string());
                        let o = self.connecting_out.clone();
                        self.try_connect_out(src, dst, &slot, o);
                    } else if rect.contains(pt) && !cards.iter().any(|c| c.contains(pt)) {
                        self.link_out = self.connecting_out.clone();
                        self.open_search(pt, Some(src));
                    }
                }
                self.connecting = None;
                self.connecting_out = None;
            }
        }

        // reverse connection: dragged out of an input port towards an output port
        if let Some((dst, slot)) = self.connecting_in.clone() {
            let target = ports.iter().find(|(i, s, _)| *i == dst && *s == slot.as_str()).map(|p| p.2);
            if let (Some(pt), Some(ip)) = (pointer, target) {
                link(&painter, pt, ip, Color32::from_rgb(255, 190, 70), 2.0);
            }
            if released {
                if let Some(pt) = pointer {
                    let near = out_ports.iter().filter(|(_, _, p)| (*p - pt).length() < 14.0 * z).min_by(|a, b| (a.2 - pt).length().partial_cmp(&(b.2 - pt).length()).unwrap_or(std::cmp::Ordering::Equal));
                    if let Some((src, oname, _)) = near {
                        let (src, oname) = (*src, oname.clone());
                        self.try_connect_out(src, dst, &slot, oname);
                    } else if rect.contains(pt) && !cards.iter().any(|c| c.contains(pt)) {
                        self.open_search_to(pt, dst, slot.clone());
                    }
                }
                self.connecting_in = None;
            }
        }

        // rubber-band selection
        if let Some(start) = self.rubber {
            if let Some(pt) = pointer {
                let r = Rect::from_two_pos(start, pt);
                painter.rect(r, 0.0, Color32::from_rgba_unmultiplied(255, 160, 40, 28), Stroke::new(1.0_f32, Color32::from_rgb(255, 160, 40)));
                if released {
                    for (s, c) in snaps.iter().zip(&cards) {
                        if r.intersects(*c) {
                            self.sel.insert(s.id);
                        }
                    }
                    if self.active.is_none() {
                        self.active = self.sel.iter().next().copied();
                        self.dirty = true;
                    }
                    self.rubber = None;
                }
            } else if released {
                self.rubber = None;
            }
        }

        // background context menu
        let click_pos = pointer;
        resp.context_menu(|ui| {
            if ui.button("Add node...   Tab").clicked() {
                acts.push(Act::Search(click_pos));
                ui.close_menu();
            }
            if ui.add_enabled(!self.clip.is_empty(), egui::Button::new("Paste")).clicked() {
                acts.push(Act::Paste(click_pos.map(|p| self.screen_to_world(p))));
                ui.close_menu();
            }
            if ui.button("Add comment frame   C").clicked() {
                acts.push(Act::Comment);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Select all").clicked() {
                acts.push(Act::SelectAll);
                ui.close_menu();
            }
            if ui.button("Frame all   F").clicked() {
                acts.push(Act::FrameAll);
                ui.close_menu();
            }
            if ui.button("Auto-layout").clicked() {
                acts.push(Act::AutoLayout);
                ui.close_menu();
            }
        });

        if self.g.nodes.is_empty() {
            painter.text(rect.center(), Align2::CENTER_CENTER, "Press Tab to add a node", FontId::proportional(16.0), Color32::from_gray(130));
        }
        self.acts.extend(acts);
    }

    // ------------------------------------------------------------ inspector
    // ------------------------------------------------------------ actions
    fn do_act(&mut self, a: Act) {
        match a {
            Act::Open => self.open_file(),
            Act::Save => self.save(),
            Act::SaveAs => self.save_as(),
            Act::Publish => self.publish(),
            Act::ImportBitmap => self.import_bitmap(),
            Act::Export => self.export.open = true,
            Act::Bake => self.bake.open = true,
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::Copy => self.copy(),
            Act::Cut => {
                self.copy();
                self.delete_selected();
            }
            Act::Paste(at) => self.paste(at),
            Act::Duplicate => {
                self.copy();
                self.paste(None);
            }
            Act::Delete => self.delete_selected(),
            Act::SelectAll => {
                self.sel = self.g.nodes.keys().copied().collect();
                self.active = self.sel.iter().next().copied();
                self.dirty = true;
            }
            Act::Comment => self.add_comment(),
            Act::Subgraph => self.make_subgraph(),
            Act::EnterSub(n) => {
                if self.parents.iter().any(|(_, name)| *name == n) {
                    return;
                }
                if !self.g.subgraphs.contains_key(&n) {
                    self.goto_root();
                }
                self.enter_sub(&n);
            }
            Act::ExitSub => self.exit_sub(),
            Act::AutoLayout => {
                self.snap();
                auto_layout(&mut self.g);
                self.fit_view();
            }
            Act::FrameAll => self.fit_view(),
            Act::Search(p) => {
                let p = p.unwrap_or_else(|| self.canvas_rect.center());
                self.open_search(p, None);
            }
            Act::NewGraph => {
                let dest = if self.screen == Screen::Editor { self.ws_init(); Some(self.pkg_sel) } else { None };
                self.open_new_dialog(dest);
            }
            Act::Home => self.screen = Screen::Home,
            Act::Drop(key, p) => {
                let world = self.screen_to_world(p);
                let w = pos2(world.x - CARD * 0.5, world.y - CARD_H * 0.5);
                if let Some(e) = self.entries().into_iter().find(|e| e.label == key) {
                    self.spawn(&e, w, None);
                }
            }
            Act::ImportGraph(path, p, gn) => {
                let at = p.map(|p| {
                    let w = self.screen_to_world(p);
                    pos2(w.x - CARD * 0.5, w.y - CARD_H * 0.5)
                });
                self.import_graph_file(path, at, gn);
            }
            Act::ImportFromPkg(i, gn, p) => {
                let at = p.map(|p| {
                    let w = self.screen_to_world(p);
                    pos2(w.x - CARD * 0.5, w.y - CARD_H * 0.5)
                });
                self.import_from_pkg(i, gn, at);
            }
            Act::NewPackage => self.open_new_dialog(None),
            Act::NewGraphIn(i) => self.open_new_dialog(Some(i)),
            Act::SwitchPkg(i) => self.switch_pkg(i),
            Act::SavePkg(i) => {
                self.switch_pkg(i);
                self.save();
            }
            Act::ClosePkg(i) => {
                if self.pkg_dirty(i) {
                    self.confirm_close = Some(i);
                } else {
                    self.close_pkg(i);
                }
            }
            Act::NewOutput(usage, from) => self.new_output(&usage, from),
        }
    }

    fn search_ui(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.search.take() else { return };
        let q = st.text.to_lowercase();
        let rank = |c: &str| ["Utility", "Generators", "Filters", "Material", "SDF", "Subgraphs"].iter().position(|x| *x == c).unwrap_or(9);
        let mut list: Vec<(u8, Entry)> = self
            .entries()
            .into_iter()
            .filter_map(|e| {
                let l = e.label.to_lowercase();
                let s = if q.is_empty() {
                    1
                } else if l.starts_with(&q) {
                    0
                } else if l.contains(&q) {
                    1
                } else if e.doc.to_lowercase().contains(&q) {
                    2
                } else {
                    return None;
                };
                Some((s, e))
            })
            .collect();
        list.sort_by(|a, b| (a.0, rank(a.1.cat), &a.1.label).cmp(&(b.0, rank(b.1.cat), &b.1.label)));
        let (down, up, enter, esc, pgdn, pgup) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.consume_key(Modifiers::NONE, Key::PageDown),
                i.consume_key(Modifiers::NONE, Key::PageUp),
            )
        });
        let moved = down || up || pgdn || pgup;
        if down {
            st.idx += 1;
        }
        if pgdn {
            st.idx += 10;
        }
        if up {
            st.idx = st.idx.saturating_sub(1);
        }
        if pgup {
            st.idx = st.idx.saturating_sub(10);
        }
        st.idx = st.idx.min(list.len().saturating_sub(1));
        let mut chosen: Option<Entry> = None;
        let mut close = esc;
        let screen = ctx.screen_rect();
        let pos = pos2(st.screen.x.min(screen.right() - 330.0).max(4.0), st.screen.y.min(screen.bottom() - 420.0).max(4.0));
        let max_h = (screen.bottom() - pos.y - 120.0).clamp(160.0, 520.0);
        let area = egui::Area::new(Id::new("node_search")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(310.0);
                let r = ui.add(egui::TextEdit::singleline(&mut st.text).hint_text("Search nodes - Enter to add").desired_width(f32::INFINITY));
                r.request_focus();
                if r.changed() {
                    st.idx = 0;
                }
                ui.separator();
                if list.is_empty() {
                    ui.small("no matching nodes");
                }
                egui::ScrollArea::vertical().max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
                    for (i, (_, e)) in list.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
                        if i == st.idx {
                            ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(60, 92, 150));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(58, 58, 58));
                        }
                        let ir = Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(16.0, 16.0));
                        ui.painter().rect_filled(ir, 3.0, kind_color(&e.kind));
                        draw_icon(ui.painter(), ir.center(), entry_icon(&e.kind), Color32::WHITE.gamma_multiply(0.9));
                        ui.painter().text(rect.left_center() + vec2(26.0, 0.0), Align2::LEFT_CENTER, &e.label, FontId::proportional(12.5), Color32::from_gray(230));
                        ui.painter().text(rect.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, e.cat, FontId::proportional(10.0), Color32::from_gray(130));
                        if resp.clicked() {
                            chosen = Some(e.clone());
                        }
                        if i == st.idx && moved {
                            resp.scroll_to_me(None);
                        }
                    }
                });
                ui.separator();
                match list.get(st.idx) {
                    Some((_, e)) => {
                        ui.small(&e.doc);
                    }
                    None => {}
                }
                ui.small(format!("{} nodes  -  scroll or use arrow keys", list.len()));
            });
        });
        if enter && chosen.is_none() {
            chosen = list.get(st.idx).map(|x| x.1.clone());
        }
        if !st.fresh {
            let clicked_outside = ctx.input(|i| i.pointer.any_click()) && ctx.input(|i| i.pointer.interact_pos()).map(|p| !area.response.rect.contains(p)).unwrap_or(false);
            if clicked_outside {
                close = true;
            }
        }
        st.fresh = false;
        if let Some(e) = chosen {
            let (w, l) = (st.world, st.link_from);
            let new_id = self.spawn(&e, w, l);
            if let (Some(id), Some((dst, slot))) = (new_id, st.link_to.clone()) {
                if e.kind != "output" {
                    self.try_connect(id, dst, &slot);
                }
            }
            close = true;
        }
        if !close {
            self.search = Some(st);
        }
    }

    fn windows(&mut self, ctx: &egui::Context) {
        if self.export.open {
            let mut open = true;
            let outputs: Vec<(String, String)> = {
                let root = self.parents.first().map(|p| &p.0).unwrap_or(&self.g);
                root.output_nodes().into_iter().map(|(u, i, _)| (usage_label(&u), i)).collect()
            };
            egui::Window::new("Export textures").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Choose folder...").clicked() {
                        if let Some(d) = rfd::FileDialog::new().pick_folder() {
                            self.export.dir = Some(d);
                        }
                    }
                    ui.label(self.export.dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| "(none)".into()));
                });
                ui.horizontal(|ui| {
                    ui.label("File prefix");
                    ui.text_edit_singleline(&mut self.export.prefix);
                });
                ui.horizontal(|ui| {
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("exp_fmt").selected_text(self.export.fmt.name()).show_ui(ui, |ui| {
                        for f in [Format::Png, Format::Jpeg, Format::Tiff] {
                            ui.selectable_value(&mut self.export.fmt, f, f.name());
                        }
                    });
                    if self.export.fmt == Format::Jpeg {
                        ui.add(egui::Slider::new(&mut self.export.quality, 1..=100).text("quality"));
                    } else {
                        ui.checkbox(&mut self.export.depth16, "16-bit");
                    }
                });
                ui.label("Resolutions (one sub-folder each when several are selected):");
                ui.horizontal(|ui| {
                    for (i, s) in [256, 512, 1024, 2048, 4096, 8192].iter().enumerate() {
                        ui.checkbox(&mut self.export.sizes[i], s.to_string());
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong("Outputs to export");
                    if ui.small_button("All").clicked() {
                        self.export.skip.clear();
                    }
                    if ui.small_button("None").clicked() {
                        self.export.skip = outputs.iter().map(|o| o.1.clone()).collect();
                    }
                });
                if outputs.is_empty() {
                    ui.colored_label(Color32::from_rgb(255, 150, 120), "This graph has no Output nodes yet.");
                }
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (usage, ident) in &outputs {
                        let mut on = !self.export.skip.contains(ident);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut on, ident.as_str()).changed() {
                                if on {
                                    self.export.skip.remove(ident);
                                } else {
                                    self.export.skip.insert(ident.clone());
                                }
                            }
                            if usage != ident {
                                ui.small(format!("({usage})"));
                            }
                        });
                    }
                });
                ui.add_space(4.0);
                if ui.button("Export").clicked() {
                    self.run_export();
                }
            });
            self.export.open = open;
        }
        if self.bake.open {
            let mut open = true;
            egui::Window::new("Mesh map baker").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                let busy = self.bake.job.is_some();
                let pick = |label: &str, slot: &mut Option<PathBuf>, ui: &mut egui::Ui| {
                    ui.horizontal(|ui| {
                        if ui.button(label).clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Wavefront OBJ", &["obj"]).pick_file() {
                                *slot = Some(p);
                            }
                        }
                        ui.label(slot.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(none)".into()));
                    });
                };
                pick("Low-poly mesh (.obj, needs UVs)", &mut self.bake.low, ui);
                pick("High-poly mesh (.obj)", &mut self.bake.high, ui);
                ui.horizontal(|ui| {
                    if ui.button("Output folder...").clicked() {
                        if let Some(d) = rfd::FileDialog::new().pick_folder() {
                            self.bake.out = Some(d);
                        }
                    }
                    ui.label(self.bake.out.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(none)".into()));
                });
                if ui.button("Use demo sphere meshes").clicked() {
                    let d = std::env::temp_dir().join("materialcraft_demo");
                    let _ = std::fs::create_dir_all(&d);
                    let (lo, hi) = (d.join("low.obj"), d.join("high.obj"));
                    if std::fs::write(&lo, bake::uv_sphere_obj(24, 16, 1.0, 0.0)).is_ok() && std::fs::write(&hi, bake::uv_sphere_obj(160, 100, 1.0, 0.04)).is_ok() {
                        self.bake.low = Some(lo);
                        self.bake.high = Some(hi);
                        if self.bake.out.is_none() {
                            self.bake.out = Some(d.join("baked"));
                        }
                    }
                }
                ui.separator();
                egui::ComboBox::from_label("Resolution").selected_text(self.bake.s.size.to_string()).show_ui(ui, |ui| {
                    for s in [256usize, 512, 1024, 2048, 4096] {
                        ui.selectable_value(&mut self.bake.s.size, s, s.to_string());
                    }
                });
                ui.add(egui::Slider::new(&mut self.bake.s.max_dist, 0.005..=0.5).text("Max ray distance (x model size)"));
                ui.add(egui::Slider::new(&mut self.bake.s.ao_rays, 4..=256).text("AO / thickness rays"));
                ui.add(egui::Slider::new(&mut self.bake.s.ao_dist, 0.02..=1.0).text("AO / thickness distance"));
                ui.add(egui::Slider::new(&mut self.bake.s.padding, 0..=32).text("Edge padding (px)"));
                ui.small("Bakes: normal, world normal, position, ambient occlusion, thickness, curvature, ID.");
                if ui.add_enabled(!busy, egui::Button::new(if busy { "Baking..." } else { "Bake" })).clicked() {
                    self.start_bake();
                }
                if !self.bake.msg.is_empty() {
                    ui.label(&self.bake.msg);
                }
            });
            self.bake.open = open;
        }
        if self.show_about {
            let mut open = true;
            egui::Window::new("Keyboard shortcuts").open(&mut open).collapsible(false).show(ctx, |ui| {
                for (k, d) in [
                    ("Tab", "add a node at the cursor (search)"),
                    ("Drag link to empty space", "add a node and auto-connect"),
                    ("Ctrl+Z / Ctrl+Y", "undo / redo"),
                    ("Ctrl+C / X / V / D", "copy / cut / paste / duplicate"),
                    ("Ctrl+A", "select all"),
                    ("Shift/Ctrl + click", "toggle selection"),
                    ("Drag background", "rubber-band select"),
                    ("Middle-drag, Alt+drag, Space+drag", "pan"),
                    ("Wheel", "zoom"),
                    ("F", "frame all"),
                    ("C", "comment frame around selection"),
                    ("Delete", "delete selection"),
                    ("Double-click subgraph", "open it"),
                    ("Ctrl+S / Ctrl+O", "save / open"),
                ] {
                    ui.horizontal(|ui| {
                        ui.strong(k);
                        ui.label(d);
                    });
                }
            });
            self.show_about = open;
        }
    }
}

