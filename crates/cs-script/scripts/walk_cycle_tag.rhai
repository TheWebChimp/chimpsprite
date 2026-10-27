// Tres copias del frame actual después de él y un tag "Walk" con los cuatro.
let first = current_frame();
for i in 0..3 {
    add_frame(#{ duplicate: first + i });
}
set_tag(#{ name: "Walk", from: first, to: first + 3, color: "#e27285" });
print(`Walk: frames ${first + 1} to ${first + 4}`);
