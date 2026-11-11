//! A CPU's processor resources and what each instruction form puts on them, from the `[schedule]` tables
//! of its `machine/tune/` file: the pin's `ProcResource` units and a form's `WriteRes` micro-ops and
//! `ReleaseAtCycles` (X86SchedSandyBridge.td).
//!
//! ```toml
//! [schedule.resources]   # name = units
//! p015 = 3
//! [schedule.forms]       # form = { uops = N, <resource> = cycles, ... }
//! alu_rr = { uops = 1, p015 = 1 }
//! ```

use llrm_mir::target::{OperationCosts, Resource, Throughput};

/// The resources and forms of one CPU's file.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Schedule {
    /// Resource name and units, in file order.
    pub resources: Vec<(String, i64)>,
    /// Form, its micro-ops, and the cycles it puts on each resource it uses (by index into `resources`).
    pub forms: Vec<(String, i64, Vec<(usize, i64)>)>,
}

impl Schedule {
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc: toml::Table = text.parse().map_err(|e| format!("schedule: {e}"))?;
        let Some(schedule) = doc.get("schedule").and_then(toml::Value::as_table) else { return Ok(Self::default()) };
        let mut out = Self::default();
        if let Some(resources) = schedule.get("resources").and_then(toml::Value::as_table) {
            for (name, units) in resources {
                let units = units.as_integer().ok_or(format!("schedule.resources.{name} is no integer"))?;
                out.resources.push((name.clone(), units));
            }
        }
        if let Some(forms) = schedule.get("forms").and_then(toml::Value::as_table) {
            for (form, row) in forms {
                let row = row.as_table().ok_or(format!("schedule.forms.{form} is no table"))?;
                let mut uops = 1;
                let mut uses = Vec::new();
                for (key, value) in row {
                    let number = value.as_integer().ok_or(format!("schedule.forms.{form}.{key} is no integer"))?;
                    if key == "uops" {
                        uops = number;
                    } else {
                        let at = out.resources.iter().position(|(name, _)| name == key).ok_or(format!("schedule.forms.{form}: no resource `{key}`"))?;
                        uses.push((at, number));
                    }
                }
                out.forms.push((form.clone(), uops, uses));
            }
        }
        Ok(out)
    }

    /// The processor resources of `issue_width` micro-ops a cycle and the file's, as operations: `operations`
    /// turns a price of each instruction form into a price of each operation (the target's mapping), once for
    /// the micro-ops and once for each resource, a form the file does not list occupying none and being one
    /// micro-op. Latencies are `latency`'s, in thousandths of a clock.
    pub fn throughput(
        &self,
        issue_width: i64,
        latency: OperationCosts,
        operations: &dyn Fn(&dyn Fn(&str) -> i64) -> OperationCosts,
    ) -> Throughput {
        let form = |name: &str| self.forms.iter().find(|(one, ..)| one == name);
        let issue = Resource { units: issue_width, costs: operations(&|name| 1000 * form(name).map_or(1, |(_, uops, _)| *uops)) };
        let mut resources = vec![issue];
        for (at, (_, units)) in self.resources.iter().enumerate() {
            let cycles = |name: &str| form(name).map_or(0, |(_, _, uses)| uses.iter().filter(|(used, _)| *used == at).map(|(_, cycles)| 1000 * cycles).sum());
            resources.push(Resource { units: *units, costs: operations(&cycles) });
        }
        Throughput { latency, resources }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_names_its_micro_ops_and_the_cycles_it_holds_on_each_resource() {
        let text = "[schedule.resources]\np015 = 3\np4 = 1\n[schedule.forms]\nalu_rr = { uops = 1, p015 = 1 }\nmov_mr = { uops = 2, p4 = 1 }\n";
        let schedule = Schedule::parse(text).expect("parses");
        assert_eq!(schedule.resources, vec![("p015".to_owned(), 3), ("p4".to_owned(), 1)]);
        assert_eq!(schedule.forms, vec![("alu_rr".to_owned(), 1, vec![(0, 1)]), ("mov_mr".to_owned(), 2, vec![(1, 1)])]);
        assert!(Schedule::parse("[schedule.forms]\nx = { uops = 1, p9 = 1 }\n").is_err());
    }
}
