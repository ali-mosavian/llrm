# Modules: this file is `main`; `import shapes.geometry` reads
# shapes/geometry.mod beside it. Imported names stay qualified.

import shapes.geometry
import report

fn main() -> i16:
    let p = shapes.geometry.Point(x=-3, y=4)
    print(report.describe(p))
    let q = p.shifted(5)
    print(report.describe(q))
    return 0
