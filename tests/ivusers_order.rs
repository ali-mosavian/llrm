//! LSR's tie between equally used registers follows the pin's user order.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_cc().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const NBODY: &str = r#"// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Six-body integrator in float (the fixed-point nbody_fixed state as floats), 1000 steps. */
/* Overdamped: the bodies settle at their centre of mass whatever the intermediate precision (a lighter damping was chaotic: #360). */

#define BODIES 6

static float pos_x[BODIES], pos_y[BODIES], vel_x[BODIES], vel_y[BODIES];

long bench_nbody_single(void)
{
    float delta_x, delta_y, dist2, falloff, acc_x, acc_y, sum;
    int body, other, step;

    for (body = 0; body < BODIES; ++body) {
        pos_x[body] = (float)(body * 7 - 15);
        pos_y[body] = (float)(body * 5 - 12);
        vel_x[body] = 0.0f;
        vel_y[body] = 0.0f;
    }
    for (step = 0; step < 1000; ++step) {
        for (body = 0; body < BODIES; ++body) {
            acc_x = 0.0f;
            acc_y = 0.0f;
            for (other = 0; other < BODIES; ++other) {
                if (other != body) {
                    delta_x = pos_x[other] - pos_x[body];
                    delta_y = pos_y[other] - pos_y[body];
                    dist2 = delta_x * delta_x + delta_y * delta_y + 1.0f;
                    falloff = 1.0f / (dist2 + 1.0f);
                    acc_x = acc_x + delta_x * falloff;
                    acc_y = acc_y + delta_y * falloff;
                }
            }
            vel_x[body] = vel_x[body] + acc_x;
            vel_y[body] = vel_y[body] + acc_y;
            vel_x[body] = vel_x[body] - vel_x[body] / 8.0f;
            vel_y[body] = vel_y[body] - vel_y[body] / 8.0f;
        }
        for (body = 0; body < BODIES; ++body) {
            pos_x[body] = pos_x[body] + vel_x[body];
            pos_y[body] = pos_y[body] + vel_y[body];
        }
    }
    sum = 0.0f;
    for (body = 0; body < BODIES; ++body)
        sum = sum + (float)(body + 1) * (pos_x[body] + 3.0f * pos_y[body] + 5.0f * vel_x[body] + 7.0f * vel_y[body]);
    return (long)(sum * 1024.0f + 0.5f);
}

"#;

/// nbody_single's force loop walked `{0,+,4}` and compared it with 24 where the
/// pin counts `{-24,+,4}` up to zero: LSR visited the counter's users in
/// use-list order (edit history), which broke its tie between equally used
/// registers the other way (IU1).
#[test]
fn the_force_loop_counts_a_pointer_from_minus_24_to_zero() {
    let text = listing(NBODY, &["-m32", "-mabi=sysv", "-O2", "-march=i486"]);
    assert!(text.contains(", -24"), "{text}");
    assert!(!text.contains(", 24\n"), "{text}");
}
