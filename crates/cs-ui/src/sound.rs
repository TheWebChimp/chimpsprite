//! Los sonidos de la interfaz: los de [UI SFX](https://github.com/romainsimon/uisfx) (CC0, en
//! `assets/ui/sounds/{estilo}/{sonido}.mp3`), solo para clics y avisos. Apagados de fábrica; en
//! Preferences se prenden y se elige el estilo y el volumen.
//!
//! Los clics no se marcan widget por widget: al final de cada frame se leen los eventos que egui
//! registra para la accesibilidad (`OutputEvent::Clicked`, con el tipo de widget) y los avisos los
//! pide [`play`] (o [`notice`], por el título del mensaje). Suena uno por frame: el más importante.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui::{self, WidgetType, output::OutputEvent};
use serde::{Deserialize, Serialize};

/// Un sonido, de menos a más importante (en un frame gana el último).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cue {
    /// Un botón (y los ítems de los menús).
    Press,
    /// Abrir un select.
    Open,
    /// Elegir una opción.
    Select,
    ToggleOff,
    ToggleOn,
    /// El aviso "Copied".
    Copy,
    Success,
    Warning,
    Error,
}

impl Cue {
    const ALL: [Cue; 9] = [
        Cue::Press,
        Cue::Open,
        Cue::Select,
        Cue::ToggleOff,
        Cue::ToggleOn,
        Cue::Copy,
        Cue::Success,
        Cue::Warning,
        Cue::Error,
    ];
}

/// El estilo de los sonidos (los 12 de UI SFX).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundPack {
    Minimal,
    Soft,
    Glass,
    #[default]
    Arcade,
    Mechanical,
    Organic,
    Dreamy,
    Scifi,
    Rubber,
    Cinematic,
    Studio,
    Zen,
}

impl SoundPack {
    pub const ALL: [SoundPack; 12] = [
        SoundPack::Minimal,
        SoundPack::Soft,
        SoundPack::Glass,
        SoundPack::Arcade,
        SoundPack::Mechanical,
        SoundPack::Organic,
        SoundPack::Dreamy,
        SoundPack::Scifi,
        SoundPack::Rubber,
        SoundPack::Cinematic,
        SoundPack::Studio,
        SoundPack::Zen,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SoundPack::Minimal => "Minimal",
            SoundPack::Soft => "Soft",
            SoundPack::Glass => "Glass",
            SoundPack::Arcade => "Arcade",
            SoundPack::Mechanical => "Mechanical",
            SoundPack::Organic => "Organic",
            SoundPack::Dreamy => "Dreamy",
            SoundPack::Scifi => "Sci-Fi",
            SoundPack::Rubber => "Rubber",
            SoundPack::Cinematic => "Cinematic",
            SoundPack::Studio => "Studio",
            SoundPack::Zen => "Zen",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(3)
    }
}

/// Los MP3 de un estilo, en el orden de [`Cue::ALL`].
macro_rules! pack {
    ($name:literal) => {
        [
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/press.mp3")).as_slice(),
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/open.mp3")).as_slice(),
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/select.mp3")).as_slice(),
            include_bytes!(concat!(
                "../../../assets/ui/sounds/",
                $name,
                "/toggle-off.mp3"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../../assets/ui/sounds/",
                $name,
                "/toggle-on.mp3"
            ))
            .as_slice(),
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/copy.mp3")).as_slice(),
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/success.mp3")).as_slice(),
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/warning.mp3")).as_slice(),
            include_bytes!(concat!("../../../assets/ui/sounds/", $name, "/error.mp3")).as_slice(),
        ]
    };
}

/// En el orden de [`SoundPack::ALL`].
static SOUNDS: [[&[u8]; 9]; 12] = [
    pack!("minimal"),
    pack!("soft"),
    pack!("glass"),
    pack!("arcade"),
    pack!("mechanical"),
    pack!("organic"),
    pack!("dreamy"),
    pack!("scifi"),
    pack!("rubber"),
    pack!("cinematic"),
    pack!("studio"),
    pack!("zen"),
];

fn sound(pack: SoundPack, cue: Cue) -> &'static [u8] {
    let i = Cue::ALL.iter().position(|c| *c == cue).unwrap_or(0);
    SOUNDS[pack.index()][i]
}

/// Si están prendidos (lo copia [`end_frame`] de las preferencias en cada frame).
static ENABLED: AtomicBool = AtomicBool::new(false);
/// El sonido pedido en este frame (el más importante).
static PENDING: Mutex<Option<Cue>> = Mutex::new(None);

pub const DEFAULT_VOLUME: u8 = 60;

/// Pide `cue` para este frame: suena al final, si es el más importante de los pedidos.
pub fn play(cue: Cue) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if pending.is_none_or(|p| cue > p) {
        *pending = Some(cue);
    }
}

/// El sonido de un mensaje por su título: "Could not…" o "…failed" es un error; con
/// advertencias, sin algo o con algo que falta, una advertencia; lo demás, un éxito.
pub fn notice_cue(title: &str) -> Cue {
    let lower = title.to_lowercase();
    if lower.starts_with("could not") || lower.contains("failed") || lower.contains("not found") {
        Cue::Error
    } else if ["warning", "without", "missing", "applied", "unsaved"]
        .iter()
        .any(|w| lower.contains(w))
    {
        Cue::Warning
    } else {
        Cue::Success
    }
}

/// Pide el sonido de un mensaje con `title`.
pub fn notice(title: &str) {
    play(notice_cue(title));
}

/// El sonido de un clic en un widget de tipo `typ` (`selected`: cómo quedó una casilla).
fn click_cue(typ: WidgetType, selected: Option<bool>) -> Option<Cue> {
    match typ {
        WidgetType::Button | WidgetType::CollapsingHeader => Some(Cue::Press),
        WidgetType::Checkbox | WidgetType::RadioButton => Some(if selected == Some(false) {
            Cue::ToggleOff
        } else {
            Cue::ToggleOn
        }),
        WidgetType::ComboBox => Some(Cue::Open),
        WidgetType::SelectableLabel | WidgetType::RadioGroup => Some(Cue::Select),
        _ => None,
    }
}

/// Al final del frame: copia las preferencias, junta los clics del frame y toca el sonido más
/// importante.
pub fn end_frame(ctx: &egui::Context, enabled: bool, pack: SoundPack, volume: u8) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
        return;
    }
    let clicks: Vec<Cue> = ctx.output(|o| {
        o.events
            .iter()
            .filter_map(|e| match e {
                OutputEvent::Clicked(info) => click_cue(info.typ, info.selected),
                _ => None,
            })
            .collect()
    });
    for cue in clicks {
        play(cue);
    }
    let cue = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(cue) = cue {
        backend::play(sound(pack, cue), f32::from(volume.min(100)) / 100.0);
    }
}

/// Escritorio: un hilo con la salida de audio (abierta la primera vez que algo suena) que
/// decodifica y mezcla cada sonido.
#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::sync::Mutex;
    use std::sync::mpsc::{Sender, channel};

    use rodio::Source as _;

    /// Un sonido para el hilo: el MP3 y el volumen.
    type Request = (&'static [u8], f32);

    static PLAYER: Mutex<Option<Sender<Request>>> = Mutex::new(None);

    pub fn play(bytes: &'static [u8], volume: f32) {
        let mut player = PLAYER.lock().unwrap_or_else(|e| e.into_inner());
        let sender = player.get_or_insert_with(|| {
            let (tx, rx) = channel::<Request>();
            let spawned = std::thread::Builder::new()
                .name("ui-sounds".into())
                .spawn(move || {
                    let mut sink = match rodio::DeviceSinkBuilder::open_default_sink() {
                        Ok(sink) => sink,
                        Err(e) => {
                            log::warn!("ui sounds: no audio output: {e}");
                            return;
                        }
                    };
                    sink.log_on_drop(false);
                    for (bytes, volume) in rx {
                        match rodio::Decoder::try_from(std::io::Cursor::new(bytes)) {
                            Ok(source) => sink.mixer().add(source.amplify(volume)),
                            Err(e) => log::warn!("ui sounds: {e}"),
                        }
                    }
                });
            if let Err(e) = spawned {
                log::warn!("ui sounds: {e}");
            }
            tx
        });
        // Sin salida de audio, el hilo ya terminó: no pasa nada.
        let _ = sender.send((bytes, volume));
    }
}

/// Web: un `<audio>` con el MP3 en una URL `data:`.
#[cfg(target_arch = "wasm32")]
mod backend {
    use base64::Engine as _;

    pub fn play(bytes: &'static [u8], volume: f32) {
        let url = format!(
            "data:audio/mpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        );
        if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&url) {
            audio.set_volume(f64::from(volume));
            let _ = audio.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pack_has_every_cue_as_mp3() {
        for pack in SoundPack::ALL {
            for cue in Cue::ALL {
                let bytes = sound(pack, cue);
                assert!(bytes.starts_with(b"ID3"), "{pack:?} {cue:?}");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn rodio_decodes_every_sound() {
        use rodio::Source as _;
        for pack in SoundPack::ALL {
            for cue in Cue::ALL {
                let decoder = rodio::Decoder::try_from(std::io::Cursor::new(sound(pack, cue)))
                    .unwrap_or_else(|e| panic!("{pack:?} {cue:?}: {e}"));
                assert!(decoder.sample_rate().get() > 0);
            }
        }
    }

    /// Toca un sonido en la salida de audio de la máquina (a mano: `--ignored`).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore]
    fn plays_on_the_default_output() {
        backend::play(sound(SoundPack::Arcade, Cue::Success), 0.6);
        std::thread::sleep(std::time::Duration::from_millis(800));
    }

    #[test]
    fn messages_sound_by_their_title() {
        assert_eq!(notice_cue("Could not save"), Cue::Error);
        assert_eq!(notice_cue("Export failed"), Cue::Error);
        assert_eq!(notice_cue("File not found"), Cue::Error);
        assert_eq!(notice_cue("Opened with warnings"), Cue::Warning);
        assert_eq!(notice_cue("Saved without guides"), Cue::Warning);
        assert_eq!(notice_cue("Exported"), Cue::Success);
    }

    #[test]
    fn clicks_sound_by_widget_type() {
        assert_eq!(click_cue(WidgetType::Button, None), Some(Cue::Press));
        assert_eq!(
            click_cue(WidgetType::Checkbox, Some(true)),
            Some(Cue::ToggleOn)
        );
        assert_eq!(
            click_cue(WidgetType::Checkbox, Some(false)),
            Some(Cue::ToggleOff)
        );
        assert_eq!(click_cue(WidgetType::ComboBox, None), Some(Cue::Open));
        assert_eq!(
            click_cue(WidgetType::SelectableLabel, Some(true)),
            Some(Cue::Select)
        );
        assert_eq!(click_cue(WidgetType::TextEdit, None), None);
        assert_eq!(click_cue(WidgetType::Slider, None), None);
    }

    #[test]
    fn the_most_important_cue_of_a_frame_wins() {
        assert!(Cue::Error > Cue::Warning && Cue::Warning > Cue::Success);
        assert!(Cue::Copy > Cue::Press && Cue::ToggleOn > Cue::Press);
    }
}
