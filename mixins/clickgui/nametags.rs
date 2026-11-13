// NameTags: player and dropped-item tags as HUD boxes at the projection of
// each entity's anchor (Javelin's NameTags).
//
// Horizon's own tags are cancelled while the module is on
// (`clickgui_nametags_hide`), so this is the only tag layer; that also hides
// named mobs and holograms, which have no tag here. Like the locator bar the
// projection uses `locator_camera`, the camera the game set after the last
// frame's HUD, so a tag trails the camera by one frame.
use crate::game::{Game, clickgui as m, clickgui_font as font, shield::OFFHAND_SLOT};
use horizon_client::hud::{HudAtlas, HudBatch, Rect};
use horizon_client::items::ItemStack;

/// Javelin's NAME_BG: black at 115/255.
const BACKGROUND: u32 = 0x7300_0000;
const NAME_INK: u32 = 0xFFFF_FFFF;
/// `§7`, the health brackets and the item count.
const SUFFIX_INK: u32 = 0xFFAA_AAAA;
/// `§c`, the health number.
const HEALTH_INK: u32 = 0xFFFF_5555;
/// The name box: Javelin's text height + 4.
const TAG_H: f32 = 14.0;
const PAD: f32 = 4.0;
/// The face beside the name, and the gap after it.
const HEAD: f32 = 13.0;
/// The equipment row above the name: ITEM_SIZE, ITEM_GAP, ITEM_PADDING.
const ICON: f32 = 13.0;
const ICON_GAP: f32 = 2.0;
const ICON_PAD: f32 = 3.0;
const ROW_H: f32 = 19.0;
const ROW_GAP: f32 = 3.0;
/// The 10px em's offset inside the box, so it sits on the middle line.
const TEXT_DY: f32 = 2.0;
/// The near plane segments are clipped at, in view-space blocks.
const NEAR: f32 = 0.05;
const DEFAULT_SKIN: &str = "entity/player/wide/steve";

struct Tag {
    at: [f64; 3],
    name: String,
    /// The item's stack size, shown as ` xN` past one.
    count: u32,
    /// The player's health rounded, with absorption.
    health: Option<i32>,
    kind: Kind,
}

enum Kind {
    Player {
        skin: Option<String>,
        equipment: Vec<ItemStack>,
    },
    Item(ItemStack),
}

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(m::ID) || !game.native_playing() {
        return;
    }
    let Some(config) = m::nametags() else {
        return;
    };
    if !config.players && !config.items {
        return;
    }
    let Some(view) = View::from_game(game, batch) else {
        return;
    };
    let tags = collect(game, config, view.camera());
    if tags.is_empty() {
        return;
    }
    let atlas = &game.gui.atlas;
    for tag in &tags {
        let Some([cx, top]) = view.project(tag.at) else {
            continue;
        };
        match &tag.kind {
            Kind::Player { skin, equipment } => {
                let face = config
                    .player_head
                    .then(|| skin_rect(atlas, skin.as_deref()))
                    .flatten();
                let inset = face.is_some().then_some(HEAD + PAD).unwrap_or(0.0);
                let (parts, text_w) = parts(tag);
                let width = PAD * 2.0 + inset + text_w;
                let x = cx - width / 2.0;
                game.native_fill(batch, [x, top, width, TAG_H], BACKGROUND);
                if let Some(face) = face {
                    face_quad(batch, face, x + PAD, top + (TAG_H - HEAD) / 2.0);
                }
                text(game, batch, &parts, x + PAD + inset, top + TEXT_DY);
                if !equipment.is_empty() {
                    let n = equipment.len() as f32;
                    let row_w = n * ICON + (n - 1.0) * ICON_GAP + ICON_PAD * 2.0;
                    let row_x = cx - row_w / 2.0;
                    let row_y = top - ROW_GAP - ROW_H;
                    game.native_fill(batch, [row_x, row_y, row_w, ROW_H], BACKGROUND);
                    for (i, stack) in equipment.iter().enumerate() {
                        let icon_x = row_x + ICON_PAD + i as f32 * (ICON + ICON_GAP);
                        icon(game, batch, stack, icon_x, row_y + ICON_PAD);
                    }
                }
            }
            Kind::Item(stack) => {
                let with_icon = config.item_icons;
                let inset = if with_icon { HEAD + PAD } else { 0.0 };
                let (parts, text_w) = parts(tag);
                let width = PAD * 2.0 + inset + text_w;
                let x = cx - width / 2.0;
                game.native_fill(batch, [x, top, width, TAG_H], BACKGROUND);
                if with_icon {
                    icon(game, batch, stack, x + PAD, top + (TAG_H - HEAD) / 2.0);
                }
                text(game, batch, &parts, x + PAD + inset, top + TEXT_DY);
            }
        }
    }
}

pub(super) struct View {
    view: glam::Mat4,
    tan_half: f32,
    aspect: f32,
    width: f32,
    height: f32,
    camera: [f64; 3],
}

impl View {
    /// The frame's camera, taken from `locator_camera` (one frame behind).
    pub(super) fn from_game(game: &Game<'_>, batch: &HudBatch) -> Option<Self> {
        let (camera, yaw, pitch, fov) = game.locator_camera?;
        Some(View {
            view: horizon_client::camera::view_matrix(yaw, pitch),
            tan_half: (fov * 0.5).to_radians().tan(),
            aspect: batch.width / batch.height,
            width: batch.width,
            height: batch.height,
            camera,
        })
    }

    pub(super) fn camera(&self) -> [f64; 3] {
        self.camera
    }

    /// The anchor's GUI-pixel position, unless it is behind the camera or
    /// well off screen.
    pub(super) fn project(&self, at: [f64; 3]) -> Option<[f32; 2]> {
        let p = self.relative(at);
        if -p.z <= NEAR {
            return None;
        }
        let [x, y] = self.screen(p);
        (x > -self.width && x < self.width * 2.0 && y > -self.height && y < self.height * 2.0)
            .then_some([x, y])
    }

    /// A world-space segment's GUI-pixel endpoints, clipped at the near plane
    /// so an endpoint behind the camera still draws its visible part.
    pub(super) fn project_segment(
        &self,
        start: [f64; 3],
        end: [f64; 3],
    ) -> Option<([f32; 2], [f32; 2])> {
        let mut a = self.relative(start);
        let mut b = self.relative(end);
        let da = -a.z;
        let db = -b.z;
        if da <= NEAR && db <= NEAR {
            return None;
        }
        if da <= NEAR {
            a = a.lerp(b, (NEAR - da) / (db - da));
        } else if db <= NEAR {
            b = b.lerp(a, (NEAR - db) / (da - db));
        }
        Some((self.screen(a), self.screen(b)))
    }

    /// A world-space point in view space (x right, y up, z away from the
    /// camera is negative).
    fn relative(&self, at: [f64; 3]) -> glam::Vec3 {
        self.view.transform_point3(glam::Vec3::new(
            (at[0] - self.camera[0]) as f32,
            (at[1] - self.camera[1]) as f32,
            (at[2] - self.camera[2]) as f32,
        ))
    }

    fn screen(&self, p: glam::Vec3) -> [f32; 2] {
        let depth = -p.z;
        [
            (p.x / (self.tan_half * self.aspect * depth) * 0.5 + 0.5) * self.width,
            (0.5 - p.y / (self.tan_half * depth) * 0.5) * self.height,
        ]
    }
}

/// The name as coloured pieces and their total width.
fn parts(tag: &Tag) -> (Vec<(String, u32)>, f32) {
    let mut parts = vec![(tag.name.clone(), NAME_INK)];
    if tag.count > 1 {
        parts.push((format!(" x{}", tag.count), SUFFIX_INK));
    }
    if let Some(health) = tag.health {
        parts.push((" [".to_owned(), SUFFIX_INK));
        parts.push((health.to_string(), HEALTH_INK));
        parts.push(("]".to_owned(), SUFFIX_INK));
    }
    let width = parts.iter().map(|(text, _)| font::width(text)).sum();
    (parts, width)
}

fn text(game: &Game<'_>, batch: &mut HudBatch, parts: &[(String, u32)], mut x: f32, y: f32) {
    for (part, color) in parts {
        font::draw(game, batch, part, [x, y], *color, true);
        x += font::width(part);
    }
}

/// The skin's 64x64 sprite, falling back to Steve while a downloaded skin is
/// missing from the atlas.
fn skin_rect(atlas: &HudAtlas, skin: Option<&str>) -> Option<Rect> {
    skin.and_then(|s| atlas.sprites.get(s))
        .or_else(|| atlas.sprites.get(DEFAULT_SKIN))
        .copied()
}

/// The face and the hat layer over it, as Javelin clips them: `u 8..16` and
/// `u 40..48` at `v 8..16` of the 64x64 skin.
fn face_quad(batch: &mut HudBatch, skin: Rect, x: f32, y: f32) {
    for u in [8.0, 40.0] {
        batch.rect_uv(x, y, HEAD, HEAD, sub_rect(skin, u), u32::MAX);
    }
}

fn sub_rect(skin: Rect, u: f32) -> Rect {
    let du = (skin.u1 - skin.u0) / 64.0;
    let dv = (skin.v1 - skin.v0) / 64.0;
    Rect {
        u0: skin.u0 + u * du,
        v0: skin.v0 + 8.0 * dv,
        u1: skin.u0 + (u + 8.0) * du,
        v1: skin.v0 + 16.0 * dv,
        w: 8,
        h: 8,
    }
}

/// `draw_item` at 16px, scaled about its centre down to the 13px box.
fn icon(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {
    let start = batch.vertices.len();
    game.draw_item(
        batch,
        stack,
        x - (16.0 - ICON) / 2.0,
        y - (16.0 - ICON) / 2.0,
    );
    batch.scale_since(
        start,
        x + ICON / 2.0,
        y + ICON / 2.0,
        ICON / 16.0,
        ICON / 16.0,
    );
}

fn collect(game: &Game<'_>, config: m::NameTagsConfig, camera: [f64; 3]) -> Vec<Tag> {
    let alpha = game.clock.camera_fraction();
    let mut tags = Vec::new();
    if config.players {
        // The players a server sends, then the guests this game hosts.
        for (id, name) in game.remote_player_names() {
            if let Some(mob) = game.mobs.iter().find(|m| m.id == id)
                && mob.alive()
            {
                tags.push(player_tag(mob, name, alpha, config));
            }
        }
        if let Some(host) = &game.lan {
            for guest in host
                .hub
                .guests
                .iter()
                .filter(|g| host.hub.here(g) && !g.dead)
            {
                tags.push(guest_tag(guest, alpha, config));
            }
        }
        // The player's own body shows in third person.
        if !game.camera_type.first_person()
            && let Some(mob) = &game.player_model
        {
            let health = game
                .player
                .as_ref()
                .map(|p| (p.health.health() + p.health.absorption()).round() as i32);
            tags.push(Tag {
                at: player_anchor(mob, alpha),
                name: own_name(game),
                count: 0,
                health,
                kind: Kind::Player {
                    skin: mob.skin.clone(),
                    equipment: config
                        .item_icons
                        .then(|| own_equipment(game))
                        .unwrap_or_default(),
                },
            });
        }
    }
    if config.items {
        for item in &game.dropped {
            let stack = &item.stack;
            let name = match stack.custom_name() {
                Some(name) => name.to_owned(),
                None => game.pack.names.display_item(&stack.item),
            };
            tags.push(Tag {
                at: item_anchor(item, camera, alpha),
                name,
                count: stack.count,
                health: None,
                kind: Kind::Item(stack.clone()),
            });
        }
    }
    tags
}

fn player_tag(
    mob: &horizon_client::mobs::Mob,
    name: String,
    alpha: f64,
    config: m::NameTagsConfig,
) -> Tag {
    Tag {
        at: player_anchor(mob, alpha),
        name,
        count: 0,
        health: Some(mob.health.max(0.0).round() as i32),
        kind: Kind::Player {
            skin: mob.skin.clone(),
            equipment: config
                .item_icons
                .then(|| equipment(mob))
                .unwrap_or_default(),
        },
    }
}

/// Javelin's anchor: `getBbHeight() + 0.4`, where a crouching player is 1.5
/// tall.
fn player_anchor(mob: &horizon_client::mobs::Mob, alpha: f64) -> [f64; 3] {
    let height = if mob.extra.crouching {
        1.5
    } else {
        f64::from(mob.height())
    };
    [
        lerp(mob.previous.x, mob.position.x, alpha),
        lerp(mob.previous.y, mob.position.y, alpha) + height + 0.4,
        lerp(mob.previous.z, mob.position.z, alpha),
    ]
}

/// The item's top plus a fifth of its distance from the eye in blocks, so
/// nearby items float clear of the drop.
fn item_anchor(
    item: &horizon_client::entities::ItemEntity,
    camera: [f64; 3],
    alpha: f64,
) -> [f64; 3] {
    let center = [
        lerp(item.previous.x, item.position.x, alpha),
        lerp(item.previous.y, item.position.y, alpha) + 0.125,
        lerp(item.previous.z, item.position.z, alpha),
    ];
    let distance = (0..3)
        .map(|i| (center[i] - camera[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    [center[0], center[1] + 0.125 + distance / 20.0, center[2]]
}

fn lerp(previous: f64, current: f64, alpha: f64) -> f64 {
    previous + (current - previous) * alpha
}

/// A tag for one of the players this game hosts over LAN, whose body lives
/// in the hub rather than in the mob list.
fn guest_tag(
    guest: &horizon_client::lan::guest::Guest,
    alpha: f64,
    config: m::NameTagsConfig,
) -> Tag {
    let position = [
        lerp(guest.previous_position[0], guest.position[0], alpha),
        lerp(guest.previous_position[1], guest.position[1], alpha),
        lerp(guest.previous_position[2], guest.position[2], alpha),
    ];
    Tag {
        at: [
            position[0],
            position[1] + f64::from(guest.dimensions().height) + 0.4,
            position[2],
        ],
        name: guest.name.clone(),
        count: 0,
        health: Some(guest.health.max(0.0).round() as i32),
        kind: Kind::Player {
            skin: None,
            equipment: config
                .item_icons
                .then(|| guest_equipment(guest))
                .unwrap_or_default(),
        },
    }
}

/// The guest's worn and held stacks in the same order as `equipment`.
fn guest_equipment(guest: &horizon_client::lan::guest::Guest) -> Vec<ItemStack> {
    let mut out = Vec::new();
    if let Some(stack) = guest.inventory.get(guest.selected).cloned().flatten() {
        out.push(stack);
    }
    if let Some(stack) = &guest.offhand {
        out.push(stack.clone());
    }
    out.extend(guest.armor.iter().flatten().cloned());
    out
}

/// The worn and held stacks in Javelin's order: main hand, off hand, head,
/// chest, legs, feet. A hand has only the synced id, so it is a bare stack.
fn equipment(mob: &horizon_client::mobs::Mob) -> Vec<ItemStack> {
    let extra = &mob.extra;
    let mut out = Vec::new();
    for id in [&extra.main_hand, &extra.off_hand].into_iter().flatten() {
        out.push(bare(id));
    }
    for (i, id) in [&extra.head, &extra.chest, &extra.legs, &extra.feet]
        .into_iter()
        .enumerate()
    {
        if let Some(stack) = &extra.armor[i] {
            out.push(stack.clone());
        } else if let Some(id) = id {
            out.push(bare(id));
        }
    }
    out
}

fn bare(id: &std::sync::Arc<str>) -> ItemStack {
    ItemStack {
        item: id.clone(),
        count: 1,
        damage: 0,
        extra: None,
    }
}

/// The local player's stacks: held, off hand, then the armour slots head to
/// feet (39..36).
fn own_equipment(game: &Game<'_>) -> Vec<ItemStack> {
    [
        Some(game.selected),
        Some(OFFHAND_SLOT),
        Some(39),
        Some(38),
        Some(37),
        Some(36),
    ]
    .into_iter()
    .flatten()
    .filter_map(|slot| game.stack_in(slot).cloned())
    .collect()
}

fn own_name(game: &Game<'_>) -> String {
    match game.remote.as_ref() {
        Some(remote) => remote.profile_name.clone(),
        None => game.options.player_name(),
    }
}
