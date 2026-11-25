//! Bend, Twist and Taper on whole groups (Shapes & Placement part 7c): each
//! selected group is deformed about its own upright axis over its whole
//! height, as a live feature the Inspector can change afterwards. The tool
//! with its handle (7d) and MCP (7e) build on this.

use prism_doc::Command;
use prism_ops::deform::DeformKind;

use super::Session;

pub const DEFORM_FIRST: &str = "Select the group to bend, twist or taper first";
pub const DEFORM_NOTHING: &str = "That group has no geometry of its own to bend, twist or taper";

impl Session {
    /// Bend, twist or taper every selected group by `amount` (radians for a
    /// bend or twist, a scale for a taper). Nothing changes, and the reason
    /// is shown, if any group cannot take it.
    pub fn deform_selected(&mut self, kind: DeformKind, amount: f64) -> bool {
        let ids: Vec<_> = self.groups.iter().copied().collect();
        if ids.is_empty() {
            self.notice = Some(DEFORM_FIRST);
            return false;
        }
        let mut commands = Vec::new();
        for id in ids {
            let asm = self.doc.assembly();
            let Some(def) = asm.context_def(&self.path_of(id)) else {
                continue;
            };
            let Some(spec) = asm.upright_deform(def, kind, amount) else {
                self.notice = Some(DEFORM_NOTHING);
                return false;
            };
            let model = &asm
                .def(def)
                .map_or_else(prism_topo::Model::new, |d| d.model.clone());
            if let Some(why) = spec
                .check()
                .or_else(|| prism_ops::deform::refusal(model, &spec))
            {
                self.notice = Some(why);
                return false;
            }
            commands.push(Command::AddDeform { instance: id, spec });
        }
        let command = match commands.len() {
            0 => return false,
            1 => commands.remove(0),
            _ => Command::Batch { commands },
        };
        self.cancel();
        self.notice = None;
        self.run(command)
    }
}
