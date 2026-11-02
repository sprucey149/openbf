use super::*;
use std::collections::{HashMap, HashSet};

pub(super) fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out.resize(out.len().next_multiple_of(4), 0);
    out
}

fn string(tag: &[u8; 4], text: &str) -> Vec<u8> {
    chunk(tag, &[text.as_bytes(), &[0]].concat())
}

fn floats(values: &[f32]) -> Result<Vec<u8>> {
    if values.iter().any(|v| !v.is_finite()) {
        bail!("nonfinite gameplay correction number");
    }
    Ok(values.iter().flat_map(|v| v.to_le_bytes()).collect())
}

fn properties(props: &[(u32, PropertyValue)]) -> Vec<u8> {
    props
        .iter()
        .flat_map(|(key, value)| {
            let value = match value {
                PropertyValue::Text(text) => text.clone(),
                PropertyValue::Tokens(tokens) => tokens
                    .iter()
                    .map(|token| match token {
                        Argument::Number(value) => value.to_string(),
                        Argument::Name(name) => name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            chunk(
                b"PROP",
                &[key.to_le_bytes().as_slice(), value.as_bytes(), &[0]].concat(),
            )
        })
        .collect()
}

fn donor_props(c: Option<&mungefs::Chunk<'_>>, retained: &[u32]) -> Vec<u8> {
    c.into_iter()
        .flat_map(|c| c.children_tagged(b"PROP"))
        .filter(|p| {
            p.payload.len() >= 4
                && retained.contains(&u32::from_le_bytes(p.payload[..4].try_into().unwrap()))
        })
        .flat_map(|p| chunk(b"PROP", p.payload))
        .collect()
}

fn class(target: &Class, donor: Option<&mungefs::Chunk<'_>>, table: &Table) -> Result<Vec<u8>> {
    let tag: &[u8; 4] = target
        .kind
        .as_bytes()
        .try_into()
        .map_err(|_| anyhow!("invalid class kind"))?;
    if !matches!(tag, b"entc" | b"wpnc" | b"ordc" | b"expc") {
        bail!("invalid class correction kind");
    }
    let mut body = string(b"BASE", &target.base);
    body.extend(string(b"TYPE", &target.at.name));
    body.extend(properties(&target.properties));
    let retained = [
        table.presentation_hashes.as_slice(),
        &target.preserve_hashes,
    ]
    .concat();
    body.extend(donor_props(donor, &retained));
    Ok(chunk(tag, &body))
}

fn world_record(r: &WorldRecord, source: Option<&mungefs::Chunk<'_>>) -> Result<Vec<u8>> {
    let tag: &[u8; 4] = r
        .kind
        .as_bytes()
        .try_into()
        .map_err(|_| anyhow!("invalid world record kind"))?;
    if !matches!(tag, b"inst" | b"regn" | b"Hint" | b"BARR") {
        bail!("invalid world record kind");
    }
    let mut info = Vec::new();
    if tag != b"BARR" {
        info.extend(string(b"TYPE", &r.class));
    }
    if let Some(setting) = &r.region_setting {
        info.extend(string(b"NAME", setting));
    } else if r.preserve_name_from.is_some() {
        let name = source
            .and_then(|c| c.child(b"INFO"))
            .and_then(|i| i.child(b"NAME"))
            .context("world correction's donor name is absent")?;
        info.extend(chunk(b"NAME", name.payload));
    } else {
        info.extend(string(b"NAME", &r.name));
    }
    info.extend(chunk(b"XFRM", &floats(&r.transform)?));
    if let Some(size) = r.size {
        info.extend(chunk(b"SIZE", &floats(&size)?));
    }
    if tag == b"BARR" {
        info.extend(chunk(b"FLAG", &r.flags.to_le_bytes()));
    }
    let mut record = chunk(b"INFO", &info);
    record.extend(properties(&r.properties));
    record.extend(donor_props(source, &r.preserve_hashes));
    Ok(chunk(tag, &record))
}

fn world(target: &World, donor: Option<&mungefs::Chunk<'_>>) -> Result<Vec<u8>> {
    let mut body = string(b"NAME", &target.at.name);
    if let Some(n) = &target.terrain_name {
        body.extend(string(b"TNAM", n));
    }
    if let Some(n) = &target.sky_name {
        body.extend(string(b"SNAM", n));
    }
    let source_records: Vec<_> = donor
        .into_iter()
        .flat_map(|c| c.children().flatten())
        .filter(|c| matches!(&c.tag, b"inst" | b"regn" | b"Hint" | b"BARR"))
        .collect();
    let mut records = Vec::new();
    if target.patch_records {
        let mut used = HashSet::new();
        for (kind, index) in &target.record_order {
            if !used.insert((kind, index)) {
                bail!("duplicate world record correction identity");
            }
            match kind.as_str() {
                "donor" => {
                    let c = source_records
                        .get(*index)
                        .context("world donor record index out of bounds")?;
                    records.push((
                        String::from_utf8_lossy(&c.tag).into_owned(),
                        chunk(&c.tag, c.payload),
                    ));
                }
                "target" => {
                    let r = target
                        .records
                        .get(*index)
                        .context("world target record index out of bounds")?;
                    let source = if let Some(index) = r.preserve_name_from {
                        Some(
                            source_records
                                .get(index)
                                .context("world name source index out of bounds")?,
                        )
                    } else {
                        source_records
                            .iter()
                            .filter(|c| {
                                String::from_utf8_lossy(&c.tag) == r.kind
                                    && c.child(b"INFO")
                                        .and_then(|i| i.child_string(b"NAME"))
                                        .as_deref()
                                        == Some(&r.name)
                            })
                            .nth(r.ordinal)
                    };
                    records.push((r.kind.clone(), world_record(r, source)?));
                }
                _ => bail!("unknown world record order source"),
            }
        }
    } else {
        for r in &target.records {
            records.push((r.kind.clone(), world_record(r, None)?));
        }
    }
    let mut info = Vec::new();
    info.extend((records.iter().filter(|r| r.0 == "regn").count() as u32).to_le_bytes());
    info.extend((records.iter().filter(|r| r.0 == "inst").count() as u32).to_le_bytes());
    body.extend(chunk(b"INFO", &info));
    for (_, record) in records {
        body.extend(record);
    }
    // Fields this parser does not consume remain donor-owned.
    for c in donor.into_iter().flat_map(|c| c.children().flatten()) {
        if !matches!(
            &c.tag,
            b"NAME" | b"TNAM" | b"SNAM" | b"INFO" | b"inst" | b"regn" | b"Hint" | b"BARR"
        ) {
            body.extend(chunk(&c.tag, c.payload));
        }
    }
    Ok(chunk(b"wrld", &body))
}

fn primitives(target: &Primitives) -> Result<Vec<u8>> {
    let mut info = target.at.name.as_bytes().to_vec();
    info.push(0);
    info.extend((target.records.len() as u32).to_le_bytes());
    let mut body = chunk(b"INFO", &info);
    for r in &target.records {
        body.extend(string(b"NAME", &r.name));
        if r.mask_own > 1 {
            bail!("invalid primitive mask ownership");
        }
        if r.mask_own == 1 {
            body.extend(chunk(b"MASK", &[r.mask]));
        }
        body.extend(string(b"PRNT", &r.parent));
        body.extend(chunk(b"XFRM", &floats(&r.transform)?));
        let mut data = r.kind.to_le_bytes().to_vec();
        data.extend(floats(&r.dimensions)?);
        body.extend(chunk(b"DATA", &data));
    }
    Ok(chunk(b"prim", &body))
}

fn config(nodes: &[ConfigNode]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for n in nodes {
        let count = u8::try_from(n.arguments.len()).context("too many config arguments")?;
        let mut data = n.hash.to_le_bytes().to_vec();
        data.push(count);
        let mut blob = Vec::new();
        for a in &n.arguments {
            match a {
                Argument::Number(value) => data.extend(floats(&[*value])?),
                Argument::Name(name) => {
                    data.extend((u32::from(count) * 4 + blob.len() as u32).to_le_bytes());
                    blob.extend(name.as_bytes());
                    blob.push(0);
                }
            }
        }
        data.extend((blob.len() as u32).to_le_bytes());
        data.extend(blob);
        body.extend(chunk(b"DATA", &data));
        if !n.children.is_empty() {
            body.extend(chunk(b"SCOP", &config(&n.children)?));
        }
    }
    Ok(body)
}

fn cutters(cutters: &[Cutter]) -> Result<Vec<u8>> {
    let mut body = chunk(b"INFO", &(cutters.len() as u32).to_le_bytes());
    for c in cutters {
        let mut info = (c.planes.len() as u32).to_le_bytes().to_vec();
        info.extend(floats(&c.sphere)?);
        let mut cut = chunk(b"INFO", &info);
        cut.extend(chunk(
            b"PLNS",
            &floats(&c.planes.iter().flatten().copied().collect::<Vec<_>>())?,
        ));
        body.extend(chunk(b"CUTR", &cut));
    }
    Ok(chunk(b"CUTS", &body))
}

fn terrain(target: &Terrain, donor: &mungefs::Chunk<'_>) -> Result<Vec<u8>> {
    let Some(target_cutters) = &target.cutters else {
        return Ok(chunk(&donor.tag, donor.payload));
    };
    let mut body = Vec::new();
    for c in donor.children() {
        let c = c.map_err(|e| anyhow!("{e}"))?;
        if &c.tag != b"CUTS" {
            body.extend(chunk(&c.tag, c.payload));
        }
    }
    body.extend(cutters(target_cutters)?);
    Ok(chunk(b"tern", &body))
}

fn model_bounds(target: &ModelBounds, donor: &mungefs::Chunk<'_>) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut found_info = false;
    for child in donor.children() {
        let child = child.map_err(|e| anyhow!("{e}"))?;
        match &child.tag {
            b"SPHR" => {}
            b"INFO" => {
                if child.payload.len() < 68 {
                    bail!("model bounds correction requires a complete INFO record");
                }
                let base = if child.payload.len() >= 72 { 16 } else { 12 };
                let mut info = child.payload.to_vec();
                let bounds = target.vis_box.iter().flatten().copied().collect::<Vec<_>>();
                info[base + 24..base + 48].copy_from_slice(&floats(&bounds)?);
                body.extend(chunk(b"INFO", &info));
                found_info = true;
            }
            _ => body.extend(chunk(&child.tag, child.payload)),
        }
    }
    if !found_info {
        bail!("model bounds correction target lacks INFO");
    }
    if let Some(sphere) = target.sphere {
        body.extend(chunk(b"SPHR", &floats(&sphere)?));
    }
    Ok(chunk(b"modl", &body))
}

fn named(c: &mungefs::Chunk<'_>) -> String {
    let raw = if matches!(&c.tag, b"prim" | b"skel") {
        c.child(b"INFO")
    } else {
        c.child(b"NAME").or_else(|| c.child(b"TYPE"))
    };
    let Some(raw) = raw else {
        return String::new();
    };
    if let Some(name) = raw
        .as_str()
        .filter(|name| !name.is_empty() && name.bytes().all(|b| (32..=126).contains(&b)))
    {
        return name.to_owned();
    }
    if raw.payload.len() == 4 {
        return format!(
            "hash_{}",
            raw.payload
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
    }
    String::new()
}

fn resource(tag: &[u8; 4]) -> bool {
    matches!(
        tag,
        b"entc"
            | b"wpnc"
            | b"ordc"
            | b"expc"
            | b"modl"
            | b"skel"
            | b"coll"
            | b"gmod"
            | b"tex_"
            | b"wrld"
            | b"tern"
            | b"zaa_"
            | b"zaf_"
            | b"scr_"
            | b"font"
            | b"plan"
            | b"plnp"
            | b"path"
            | b"prim"
            | b"PATH"
            | b"lght"
            | b"Locl"
            | b"mcfg"
            | b"fx__"
            | b"snd_"
            | b"anim"
            | b"bnd_"
            | b"shdw"
            | b"CLTH"
            | b"comb"
            | b"sky_"
    )
}

struct Rebuild<'a> {
    table: &'a Table,
    file: &'a File,
    counts: HashMap<(Vec<u32>, [u8; 4], String), usize>,
    applied: HashSet<(&'static str, usize)>,
}

impl<'a> Rebuild<'a> {
    fn programs(&self) -> Result<&'a program::Programs> {
        self.table
            .programs
            .as_ref()
            .context("correction table has no programs")
    }

    fn matches(&self, target: &Locator, at: &Locator) -> bool {
        target.scope == at.scope
            && target.ordinal == at.ordinal
            && target.name.eq_ignore_ascii_case(&at.name)
    }

    fn replacement(&mut self, c: &mungefs::Chunk<'_>, at: &Locator) -> Result<Option<Vec<u8>>> {
        for (i, t) in self.file.classes.iter().enumerate() {
            if t.kind.as_bytes() == c.tag && self.matches(&t.at, at) {
                self.applied.insert(("class", i));
                return Ok(Some(class(t, Some(c), self.table)?));
            }
        }
        if self
            .file
            .remove_resources
            .iter()
            .any(|t| t.kind.as_bytes() == c.tag && self.matches(&t.at, at))
        {
            return Ok(Some(Vec::new()));
        }
        for (i, target) in self.file.resources.iter().enumerate() {
            if target.kind.as_bytes() == c.tag && self.matches(&target.at, at) {
                self.applied.insert(("resource", i));
                return Ok(Some(program::resource(self.programs()?, target, Some(c))?));
            }
        }
        if self
            .file
            .remove_classes
            .iter()
            .any(|t| t.kind.as_bytes() == c.tag && self.matches(&t.at, at))
            || (&c.tag == b"prim"
                && self
                    .file
                    .remove_primitives
                    .iter()
                    .any(|t| self.matches(t, at)))
            || (&c.tag == b"coll"
                && self
                    .file
                    .remove_collision
                    .iter()
                    .any(|t| self.matches(t, at)))
        {
            return Ok(Some(Vec::new()));
        }
        macro_rules! replace {
            ($field:ident, $tag:literal, $key:literal, $build:expr) => {
                if &c.tag == $tag {
                    for (i, t) in self.file.$field.iter().enumerate() {
                        if self.matches(&t.at, at) {
                            self.applied.insert(($key, i));
                            return Ok(Some($build(t)?));
                        }
                    }
                }
            };
        }
        replace!(world, b"wrld", "world", |t| world(t, Some(c)));
        replace!(primitives, b"prim", "prim", primitives);
        if &c.tag == b"tern" {
            let mut replacement = None;
            for (i, t) in self.file.terrain.iter().enumerate() {
                if self.matches(&t.at, at) {
                    self.applied.insert(("terrain", i));
                    replacement = Some(terrain(t, c)?);
                }
            }
            for (i, target) in self.file.remove_water.iter().enumerate() {
                if self.matches(target, at) {
                    let bytes = replacement
                        .take()
                        .unwrap_or_else(|| chunk(b"tern", c.payload));
                    let source = mungefs::chunks(&bytes).next().unwrap()?;
                    if source.child(b"WATR").is_none() {
                        bail!("water removal target has no water");
                    }
                    let mut body = Vec::new();
                    for child in source.children() {
                        let child = child?;
                        if &child.tag != b"WATR" {
                            body.extend(chunk(&child.tag, child.payload));
                        }
                    }
                    replacement = Some(chunk(b"tern", &body));
                    self.applied.insert(("water", i));
                }
            }
            if replacement.is_some() {
                return Ok(replacement);
            }
        }
        replace!(sky, b"sky_", "sky", |t: &Sky| Ok::<_, anyhow::Error>(
            chunk(b"sky_", &config(&t.nodes)?)
        ));
        replace!(models, b"modl", "model", |t| model_bounds(t, c));
        if &c.tag == b"coll" {
            for (i, t) in self.file.collision.iter().enumerate() {
                if self.matches(&t.at, at) {
                    self.applied.insert(("coll", i));
                    return Ok(Some(program::collision(self.programs()?, t, Some(c))?));
                }
            }
        }
        Ok(None)
    }

    fn append_missing(&mut self, scope: &[u32], out: &mut Vec<u8>) -> Result<()> {
        macro_rules! append {
            ($field:ident, $key:literal, $build:expr) => {
                for (i, t) in self.file.$field.iter().enumerate() {
                    if t.at.scope == scope && self.applied.insert(($key, i)) {
                        out.extend($build(t)?);
                    }
                }
            };
        }
        append!(classes, "class", |t| class(t, None, self.table));
        append!(world, "world", |t| world(t, None));
        append!(primitives, "prim", primitives);
        append!(sky, "sky", |t: &Sky| Ok::<_, anyhow::Error>(chunk(
            b"sky_",
            &config(&t.nodes)?
        )));
        let programs = self.table.programs.as_ref();
        let need = || programs.context("correction table has no programs");
        append!(collision, "coll", |t| program::collision(need()?, t, None));
        append!(resources, "resource", |t| program::resource(
            need()?,
            t,
            None
        ));
        Ok(())
    }

    fn walk(&mut self, bytes: &[u8], scope: &[u32], wrapped: bool) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for c in mungefs::chunks(bytes) {
            let c = c.map_err(|e| anyhow!("{e}"))?;
            let payload = if &c.tag == b"lvl_" {
                Some(self.walk(c.payload, scope, true)?)
            } else if wrapped && !resource(&c.tag) {
                let mut next = scope.to_vec();
                next.push(u32::from_le_bytes(c.tag));
                Some(self.walk(c.payload, &next, false)?)
            } else {
                None
            };
            if let Some(payload) = payload {
                out.extend(chunk(&c.tag, &payload));
                continue;
            }
            if resource(&c.tag) {
                let name = named(&c);
                let ordinal = self
                    .counts
                    .entry((scope.to_vec(), c.tag, name.to_ascii_lowercase()))
                    .or_default();
                let at = Locator {
                    scope: scope.to_vec(),
                    name,
                    ordinal: *ordinal,
                };
                *ordinal += 1;
                if let Some(bytes) = self.replacement(&c, &at)? {
                    out.extend(bytes);
                    continue;
                }
            }
            out.extend(chunk(&c.tag, c.payload));
        }
        if !wrapped {
            self.append_missing(scope, &mut out)?;
        }
        Ok(out)
    }
}

pub(super) fn apply(bytes: &[u8], table: &Table, file: &File) -> Result<Vec<u8>> {
    let inner = mungefs::open(bytes).map_err(|e| anyhow!("{e}"))?;
    let mut rebuild = Rebuild {
        table,
        file,
        counts: HashMap::new(),
        applied: HashSet::new(),
    };
    let mut body = rebuild.walk(inner, &[], false)?;
    let scopes: HashSet<Vec<u32>> = file
        .classes
        .iter()
        .map(|t| &t.at.scope)
        .chain(file.world.iter().map(|t| &t.at.scope))
        .chain(file.primitives.iter().map(|t| &t.at.scope))
        .chain(file.sky.iter().map(|t| &t.at.scope))
        .chain(file.collision.iter().map(|t| &t.at.scope))
        .chain(file.resources.iter().map(|t| &t.at.scope))
        .cloned()
        .collect();
    let mut scopes: Vec<_> = scopes.into_iter().collect();
    scopes.sort();
    for scope in scopes {
        let mut missing = Vec::new();
        rebuild.append_missing(&scope, &mut missing)?;
        if missing.is_empty() {
            continue;
        }
        for hash in scope.iter().rev() {
            missing = chunk(b"lvl_", &chunk(&hash.to_le_bytes(), &missing));
        }
        body.extend(missing);
    }
    for (i, _) in file.terrain.iter().enumerate() {
        if !rebuild.applied.contains(&("terrain", i)) {
            bail!("terrain correction target is absent from the source");
        }
    }
    for (i, _) in file.models.iter().enumerate() {
        if !rebuild.applied.contains(&("model", i)) {
            bail!("model bounds correction target is absent from the source");
        }
    }
    for (i, _) in file.remove_water.iter().enumerate() {
        if !rebuild.applied.contains(&("water", i)) {
            bail!("water removal target is absent from the source");
        }
    }
    Ok(chunk(b"ucfb", &body))
}
