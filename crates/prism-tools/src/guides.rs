//! What the tool hint panel (Help > Tool Hint Panel) says about each tool:
//! a line on what it does, and the steps to use it. The live hint for the
//! step in progress comes from [`crate::Session::hint`].

use crate::session::{self as s, TOOLS};

/// One tool's guide.
#[derive(Clone, Copy, Debug)]
pub struct Guide {
    /// What the tool does, in one line.
    pub what: &'static str,
    /// How to use it, step by step.
    pub steps: &'static [&'static str],
}

const fn g(what: &'static str, steps: &'static [&'static str]) -> Guide {
    Guide { what, steps }
}

/// The guide for a tool the session runs.
pub fn guide(tool: &str) -> Option<Guide> {
    Some(match tool {
        s::SELECT => g(
            "Pick points, edges, faces and groups to work on.",
            &[
                "Click to select; Shift toggles, Ctrl adds.",
                "Double-click a face for it and its edges; triple-click for everything connected.",
                "Drag left to right for a window box, right to left for a crossing box.",
                "Double-click a group to open it; Esc closes it.",
            ],
        ),
        s::LINE => g(
            "Draw edges. A closed loop on a plane becomes a face.",
            &[
                "Click a start point, then each next point.",
                "Arrows lock an axis; Shift holds the current inference.",
                "Type a length and press Enter.",
                "Esc stops the line.",
            ],
        ),
        s::FREEHAND => g(
            "Draw a smooth curve by hand.",
            &[
                "Press and drag over a face, the ground or a plane.",
                "Release to finish: the stroke is fitted to a smooth curve.",
                "End where you began to close it into a face.",
                "Hold Shift as you press to draw over anything in 3D, not on one plane.",
            ],
        ),
        s::RECT => g(
            "Draw a rectangle on a face or an axis plane.",
            &[
                "Click the first corner.",
                "Click the opposite corner, or type width,height and press Enter.",
                "Arrows pick the plane.",
            ],
        ),
        s::RRECT => g(
            "Draw a rectangle at any angle, on any plane.",
            &[
                "Click the first corner, then the second: that is the first side.",
                "Click the width; snap to a point to tilt the rectangle up to it.",
                "Type a length, then a width or width,angle and press Enter.",
                "Arrows lock the first side's axis, then pick the width's plane.",
            ],
        ),
        s::CIRCLE => g(
            "Draw an exact circle.",
            &[
                "Click the centre, then a point on the radius.",
                "Type a radius and press Enter.",
                "Arrows pick the plane; double-click repeats the last size.",
            ],
        ),
        s::POLYGON => g(
            "Draw a regular polygon.",
            &[
                "Type the number of sides as 8s and press Enter.",
                "Click the centre, then a corner.",
                "Type a radius and press Enter.",
            ],
        ),
        s::ARC => g(
            "Draw an arc by its ends and its bulge.",
            &[
                "Click the start, then the end.",
                "Move to set the bulge and click; type a bulge or radius.",
            ],
        ),
        s::ARC2 => g(
            "Draw an arc from its centre.",
            &[
                "Click the centre, then the start.",
                "Click where it ends, or type an angle.",
            ],
        ),
        s::ARC3 => g(
            "Draw an arc through three points.",
            &["Click the start, a point on the arc, then the end."],
        ),
        s::ARC_CENTRE => g(
            "Draw an arc from its centre and two ends.",
            &["Click the centre, the start, then the end."],
        ),
        s::PIE => g(
            "Draw a pie slice: an arc closed to its centre.",
            &["Click the centre, the start, then the end."],
        ),
        s::CURVE => g(
            "Draw a NURBS curve from control points.",
            &[
                "Click each control point.",
                "Ctrl switches to a curve through the clicks.",
                "Double-click or Enter ends it; click the start to close it.",
            ],
        ),
        s::PUSH => g(
            "Push or pull a face into 3D.",
            &[
                "Click a face, move, then click; or type a distance.",
                "Alt pulls a new loop, leaving the old face.",
                "Double-click repeats the last distance.",
            ],
        ),
        s::MOVE => g(
            "Move the selection, or copy it.",
            &[
                "Click a point to move from, then where it goes.",
                "Ctrl copies; type 3x or /3 after a copy for an array.",
                "Arrows lock an axis; type a distance.",
                "A group or whole body lines up with its neighbours and snaps to equal gaps; Ctrl+U turns that off.",
            ],
        ),
        s::ROTATE => g(
            "Rotate the selection.",
            &[
                "Click the centre, then the start of the angle.",
                "Click the end, or type degrees; it snaps every 15 degrees.",
                "Ctrl copies.",
            ],
        ),
        s::SCALE => g(
            "Scale the selection by its grips.",
            &[
                "Drag a corner, edge or face grip.",
                "Type a factor; Alt scales about the centre.",
            ],
        ),
        s::ERASE => g(
            "Erase edges and guides.",
            &["Click or drag over what to erase. Faces meeting at it merge."],
        ),
        s::OFFSET => g(
            "Offset a face's edges inward or outward.",
            &["Click a face, move, then click; or type a distance."],
        ),
        s::TAPE => g(
            "Measure, and make guides.",
            &[
                "Click a start point, then the end.",
                "From an edge it makes a parallel guide. Ctrl measures only.",
            ],
        ),
        s::PROTRACTOR => g(
            "Make a guide at an angle.",
            &["Click the centre, then the base line, then the angle (or type it)."],
        ),
        s::FILLET => g(
            "Round or chamfer edges, with clean corners where they meet.",
            &[
                "Set the size in the Fillet window, or type it.",
                "Click an edge between two faces (a line in pieces is rounded whole).",
                "Double-click a face to round all its edges.",
                "Shift-click edges to gather them (or select them first), then Enter.",
            ],
        ),
        s::LOFT => g(
            "Fill a ring of sides, or loft through separate ones.",
            &[
                "Click an edge: the side runs on through gentle turns (Shift adds or removes one edge).",
                "Enter or Next Side finishes a side; pick the next.",
                "Sides that close a ring fill it; sides apart are lofted through, in order.",
                "Enter or Loft makes it. Esc steps back one side.",
            ],
        ),
        s::FOLLOW => g(
            "Sweep a face along a path.",
            &[
                "Select the path's edges, then click the face.",
                "Alt follows the selected face's own edge.",
            ],
        ),
        s::PAINT => g(
            "Paint faces with the chosen material.",
            &[
                "Pick a material in the Materials window, then click faces.",
                "Alt samples the material under the cursor.",
            ],
        ),
        s::TEXTURE => g(
            "Place a face's texture.",
            &["Drag the pins to move, turn and stretch it; Enter keeps it, Esc puts it back."],
        ),
        s::ORBIT => g(
            "Turn the view around the model.",
            &["Drag to orbit; Shift pans. Esc returns to your tool."],
        ),
        s::PAN => g(
            "Slide the view.",
            &["Drag to pan. Esc returns to your tool."],
        ),
        s::ZOOM => g(
            "Zoom the view.",
            &[
                "Drag up to zoom in, down to zoom out.",
                "Type a field of view in degrees and press Enter.",
            ],
        ),
        s::WINDOW => g(
            "Zoom to a box.",
            &["Drag a box around what to see. A click zooms in."],
        ),
        s::WALK => g(
            "Walk through the model.",
            &["Drag up and down to walk, left and right to turn."],
        ),
        s::LOOK => g(
            "Look around from where you stand.",
            &["Drag to turn your head."],
        ),
        _ => return None,
    })
}

/// Every tool the session runs has a guide.
pub fn missing() -> Vec<&'static str> {
    TOOLS.into_iter().filter(|t| guide(t).is_none()).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_tool_has_a_guide() {
        assert_eq!(super::missing(), Vec::<&str>::new());
    }
}
