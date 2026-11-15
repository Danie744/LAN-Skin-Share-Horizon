// LAN Skin Share for Horizon 26.3 (Nightly 2026-11-13).
// Client/server protocol for sharing a locally selected PNG during a LAN session.
// No nickname lookup, account authentication, Mojang endpoint or external server.

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const CHANNEL: &str = "lan_skin_share:sync";
const MAGIC: &[u8; 4] = b"LSS1";
const HELLO: u8 = 0x01;
const PUBLISH: u8 = 0x02;
const SNAPSHOT_BEGIN: u8 = 0x10;
const SKIN_RECORD: u8 = 0x11;
const MAX_PNG_BYTES: usize = 24_000;
const RESEND_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub(super) struct SharedSkin {
    pub(super) slim: bool,
    pub(super) png: Vec<u8>,
}

#[derive(Default)]
struct ServerSession {
    peers: HashSet<u128>,
    skins: HashMap<u128, SharedSkin>,
}

static SERVER_SESSION: OnceLock<Mutex<ServerSession>> = OnceLock::new();
static CLIENT_SKINS: OnceLock<Mutex<HashMap<u128, SharedSkin>>> = OnceLock::new();
static LAST_PUBLISH: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

fn server_session() -> &'static Mutex<ServerSession> {
    SERVER_SESSION.get_or_init(|| Mutex::new(ServerSession::default()))
}

fn client_skins() -> &'static Mutex<HashMap<u128, SharedSkin>> {
    CLIENT_SKINS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn last_publish() -> &'static Mutex<Option<Instant>> {
    LAST_PUBLISH.get_or_init(|| Mutex::new(None))
}

fn make_message(kind: u8) -> Vec<u8> {
    let mut message = Vec::with_capacity(5);
    message.extend_from_slice(MAGIC);
    message.push(kind);
    message
}

pub(super) fn valid_skin_png(bytes: &[u8]) -> bool {
    if bytes.len() < 33 || bytes.len() > MAX_PNG_BYTES {
        return false;
    }
    if &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return false;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    // The Horizon skin atlas and skin-layer builder use a 64x64 RGBA image.
    width == 64 && height == 64
}

fn encode_record(player: u128, skin: &SharedSkin) -> Option<Vec<u8>> {
    if !valid_skin_png(&skin.png) {
        return None;
    }
    let mut message = make_message(SKIN_RECORD);
    message.extend_from_slice(&player.to_be_bytes());
    message.push(u8::from(skin.slim));
    message.extend_from_slice(&skin.png);
    Some(message)
}

fn decode_record(data: &[u8]) -> Option<(u128, SharedSkin)> {
    if data.len() < 22 || &data[..4] != MAGIC || data[4] != SKIN_RECORD {
        return None;
    }
    let uuid = u128::from_be_bytes(data[5..21].try_into().ok()?);
    let slim = match data[21] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let png = data[22..].to_vec();
    if !valid_skin_png(&png) {
        return None;
    }
    Some((uuid, SharedSkin { slim, png }))
}

fn skin_directory() -> Option<PathBuf> {
    Some(crate::game::Game::native_config_dir().ok()?.join("lan_skin_share"))
}

fn encode_rgba_png(rgba: &[u8]) -> Option<Vec<u8>> {
    if rgba.len() != 64 * 64 * 4 {
        return None;
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 64, 64);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    valid_skin_png(&bytes).then_some(bytes)
}

fn read_local_skin(game: &crate::game::Game<'_>) -> Option<SharedSkin> {
    // Optional manual override. Otherwise share the exact skin Horizon has
    // already selected for this local player: Microsoft profile skin or the
    // launcher's offline skin. The mod itself performs no account/name lookup.
    if let Some(root) = skin_directory()
        && let Ok(png) = fs::read(root.join("skin.png"))
        && valid_skin_png(&png)
    {
        let slim = fs::read_to_string(root.join("model.txt"))
            .map(|value| value.trim().eq_ignore_ascii_case("slim"))
            .unwrap_or(false);
        return Some(SharedSkin { slim, png });
    }

    let model = game.player_model.as_ref()?;
    let location = model.skin.as_deref()?;
    let rgba = game.skin_downloads.borrow().image(location)?.to_vec();
    let png = encode_rgba_png(&rgba)?;
    Some(SharedSkin { slim: model.slim, png })
}

pub(super) fn publish_local_skin(game: &mut crate::game::Game<'_>) {
    // Do not attempt to publish while this Game is not connected to a server.
    if game.remote.is_none() {
        return;
    }

    // Retry every 10 seconds: the first attempts can happen before mod_net is ready.
    let now = Instant::now();
    {
        let mut guard = last_publish()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard
            .as_ref()
            .is_some_and(|last| now.duration_since(*last) < RESEND_INTERVAL)
        {
            return;
        }
        *guard = Some(now);
    }

    let _ = game.native_net_send_to_server(CHANNEL, &make_message(HELLO));

    if let Some(skin) = read_local_skin(game) {
        let mut upload = make_message(PUBLISH);
        upload.push(u8::from(skin.slim));
        upload.extend_from_slice(&skin.png);
        let _ = game.native_net_send_to_server(CHANNEL, &upload);
    }
}

pub(super) fn server_receive(
    game: &mut crate::game::Game<'_>,
    player: u128,
    channel: &str,
    data: &[u8],
) {
    if channel != CHANNEL || data.len() < 5 || &data[..4] != MAGIC {
        return;
    }

    match data[4] {
        HELLO if data.len() == 5 => {
            let snapshot = {
                let mut state = server_session()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.peers.insert(player);
                state.skins.iter().map(|(id, skin)| (*id, skin.clone())).collect::<Vec<_>>()
            };

            let _ = game.native_net_send_to_player(player, CHANNEL, &make_message(SNAPSHOT_BEGIN));
            for (owner, skin) in snapshot {
                if let Some(message) = encode_record(owner, &skin) {
                    let _ = game.native_net_send_to_player(player, CHANNEL, &message);
                }
            }
        }
        PUBLISH if data.len() >= 6 => {
            let slim = match data[5] {
                0 => false,
                1 => true,
                _ => return,
            };
            let png = data[6..].to_vec();
            if !valid_skin_png(&png) {
                return;
            }
            let skin = SharedSkin { slim, png };
            let recipients = {
                let mut state = server_session()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.peers.insert(player);
                state.skins.insert(player, skin.clone());
                state.peers.iter().copied().collect::<Vec<_>>()
            };
            if let Some(message) = encode_record(player, &skin) {
                for recipient in recipients {
                    let _ = game.native_net_send_to_player(recipient, CHANNEL, &message);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn client_receive(channel: &str, data: &[u8]) {
    if channel != CHANNEL || data.len() < 5 || &data[..4] != MAGIC {
        return;
    }

    match data[4] {
        SNAPSHOT_BEGIN if data.len() == 5 => {
            client_skins()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clear();
        }
        SKIN_RECORD => {
            if let Some((uuid, skin)) = decode_record(data) {
                client_skins()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(uuid, skin);
            }
        }
        _ => {}
    }
}

pub(super) fn client_skin_snapshot() -> HashMap<u128, SharedSkin> {
    client_skins()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(super) fn skin_location(uuid: u128, png: &[u8], slim: bool) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    png.hash(&mut hasher);
    slim.hash(&mut hasher);
    let hash = hasher.finish();
    format!("skins/lan_skin_share/{uuid:032x}/{hash:016x}")
}

pub(super) fn decode_skin_png(bytes: &[u8]) -> Option<Vec<u8>> {
    if !valid_skin_png(bytes) {
        return None;
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut frame = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut frame).ok()?;
    if info.width != 64 || info.height != 64 || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let data = &frame[..info.buffer_size()];
    let mut rgba = Vec::with_capacity(64 * 64 * 4);
    match info.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(data),
        png::ColorType::Rgb => {
            for pixel in data.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &gray in data {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in data.chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Indexed => return None,
    }
    (rgba.len() == 64 * 64 * 4).then_some(rgba)
}

pub(super) fn clear_server_session() {
    *server_session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = ServerSession::default();
    *client_skins()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    *last_publish()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}
