# Sonidos de la interfaz

Los MP3 de [UI SFX](https://github.com/romainsimon/uisfx) (`packages/uisfx/sounds/{estilo}/`),
CC0 1.0 (dominio público; ver `LICENSE-AUDIO`). Se usan solo los de clics y avisos, de los 12
estilos: `press`, `open`, `select`, `toggle-on`, `toggle-off`, `copy`, `success`, `warning` y
`error`. Van dentro del ejecutable (`crates/cs-ui/src/sound.rs`).

Son MP3 y no los Ogg del paquete: esos son Opus, que `rodio` no decodifica (y Safari no toca Ogg).
