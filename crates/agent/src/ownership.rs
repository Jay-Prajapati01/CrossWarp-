use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && x >= self.x
            && y >= self.y
            && x < self.x + self.width
            && y < self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    LocalDesktop,
    ExtendedDisplay,
    NativeOverlay,
    WindowProxy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InputOwner {
    pub device_id: String,
    pub window_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    pub id: String,
    pub kind: SurfaceKind,
    pub bounds: Rect,
    pub z_order: i32,
    pub owner: InputOwner,
    pub interactive: bool,
}

/// Hit-tests the topmost interactive surface, making ownership explicit for each input.
#[derive(Clone, Debug, Default)]
pub struct SurfaceMap {
    surfaces: Vec<Surface>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleasedInput {
    pub input: String,
    pub owner: InputOwner,
}

/// Routes keyboard and button transitions to the owner that received the down event.
/// This guarantees balanced release even if focus changes or the connection drops.
#[derive(Clone, Debug)]
pub struct InputRouter {
    surfaces: SurfaceMap,
    local_owner: InputOwner,
    focused_owner: Option<InputOwner>,
    held_inputs: BTreeMap<String, InputOwner>,
}

impl InputRouter {
    pub fn new(local_owner: InputOwner) -> Self {
        Self {
            surfaces: SurfaceMap::default(),
            local_owner,
            focused_owner: None,
            held_inputs: BTreeMap::new(),
        }
    }

    pub fn surfaces_mut(&mut self) -> &mut SurfaceMap {
        &mut self.surfaces
    }

    pub fn focus_at(&mut self, x: f64, y: f64) -> &InputOwner {
        self.focused_owner = self.surfaces.owner_at(x, y).cloned();
        self.focused_owner.as_ref().unwrap_or(&self.local_owner)
    }

    pub fn keyboard_owner(&self) -> &InputOwner {
        self.focused_owner.as_ref().unwrap_or(&self.local_owner)
    }

    pub fn key_down(&mut self, key: impl Into<String>) -> Option<InputOwner> {
        let key = format!("key:{}", key.into());
        if self.held_inputs.contains_key(&key) {
            return None;
        }
        let owner = self.keyboard_owner().clone();
        self.held_inputs.insert(key, owner.clone());
        Some(owner)
    }

    pub fn button_down(&mut self, button: u8, x: f64, y: f64) -> Option<InputOwner> {
        let button = format!("button:{button}");
        if self.held_inputs.contains_key(&button) {
            return None;
        }
        let owner = self.focus_at(x, y).clone();
        self.held_inputs.insert(button, owner.clone());
        Some(owner)
    }

    pub fn input_up(&mut self, input: &str) -> Option<InputOwner> {
        self.held_inputs.remove(input)
    }

    pub fn release_all(&mut self) -> Vec<ReleasedInput> {
        std::mem::take(&mut self.held_inputs)
            .into_iter()
            .map(|(input, owner)| ReleasedInput { input, owner })
            .collect()
    }
}

impl SurfaceMap {
    pub fn upsert(&mut self, surface: Surface) {
        if let Some(existing) = self.surfaces.iter_mut().find(|item| item.id == surface.id) {
            *existing = surface;
        } else {
            self.surfaces.push(surface);
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<Surface> {
        let index = self.surfaces.iter().position(|surface| surface.id == id)?;
        Some(self.surfaces.remove(index))
    }

    pub fn owner_at(&self, x: f64, y: f64) -> Option<&InputOwner> {
        self.surfaces
            .iter()
            .filter(|surface| surface.interactive && surface.bounds.contains(x, y))
            .max_by_key(|surface| surface.z_order)
            .map(|surface| &surface.owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(id: &str, kind: SurfaceKind, z_order: i32, owner: &str) -> Surface {
        Surface {
            id: id.into(),
            kind,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z_order,
            owner: InputOwner {
                device_id: owner.into(),
                window_token: Some(format!("{owner}-window")),
            },
            interactive: true,
        }
    }

    #[test]
    fn pointer_goes_to_highest_visible_interactive_owner() {
        let mut map = SurfaceMap::default();
        map.upsert(surface("receiver", SurfaceKind::ExtendedDisplay, 1, "jack"));
        map.upsert(surface(
            "local-overlay",
            SurfaceKind::NativeOverlay,
            2,
            "desktop-t13bkt8",
        ));
        assert_eq!(
            map.owner_at(50.0, 50.0).unwrap().device_id,
            "desktop-t13bkt8"
        );
    }

    #[test]
    fn noninteractive_overlay_does_not_steal_pointer() {
        let mut map = SurfaceMap::default();
        map.upsert(surface("receiver", SurfaceKind::ExtendedDisplay, 1, "jack"));
        let mut overlay = surface("status", SurfaceKind::NativeOverlay, 9, "desktop-t13bkt8");
        overlay.interactive = false;
        map.upsert(overlay);
        assert_eq!(map.owner_at(50.0, 50.0).unwrap().device_id, "jack");
    }

    #[test]
    fn removing_proxy_reveals_underlying_surface_owner() {
        let mut map = SurfaceMap::default();
        map.upsert(surface("receiver", SurfaceKind::ExtendedDisplay, 1, "jack"));
        map.upsert(surface(
            "proxy",
            SurfaceKind::WindowProxy,
            3,
            "desktop-t13bkt8",
        ));
        assert_eq!(
            map.owner_at(50.0, 50.0).unwrap().device_id,
            "desktop-t13bkt8"
        );
        map.remove("proxy");
        assert_eq!(map.owner_at(50.0, 50.0).unwrap().device_id, "jack");
    }

    #[test]
    fn keyboard_stays_with_owner_of_key_down_when_focus_changes() {
        let local = InputOwner {
            device_id: "jack".into(),
            window_token: None,
        };
        let mut router = InputRouter::new(local);
        router.surfaces_mut().upsert(surface(
            "proxy",
            SurfaceKind::WindowProxy,
            2,
            "desktop-t13bkt8",
        ));
        let original_owner = router.focus_at(50.0, 50.0).clone();
        assert_eq!(router.key_down("ShiftLeft"), Some(original_owner.clone()));
        assert_eq!(router.key_down("ShiftLeft"), None);
        router.focus_at(150.0, 150.0);
        assert_eq!(router.input_up("key:ShiftLeft"), Some(original_owner));
        assert_eq!(router.input_up("key:ShiftLeft"), None);
    }

    #[test]
    fn disconnect_releases_every_held_input_to_its_original_owner() {
        let local = InputOwner {
            device_id: "jack".into(),
            window_token: None,
        };
        let mut router = InputRouter::new(local);
        router.key_down("ControlLeft");
        router.button_down(1, 0.0, 0.0);
        let released = router.release_all();
        assert_eq!(released.len(), 2);
        assert!(released.iter().all(|item| item.owner.device_id == "jack"));
        assert!(router.release_all().is_empty());
    }
}
