//! GEKKO NOIR / native-only mirror and LED appearance.
//! No Electron, WebView or page-injected CSS. The user's LED color never tints the page.
use std::fs;
use std::path::PathBuf;
use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NoirSettings {
    pub led_rgb: [u8; 3],
    pub led_intensity: f32,
    pub mirror_opacity: f32,
    pub reduce_motion: bool,
}
impl Default for NoirSettings {
    fn default() -> Self {
        Self {
            led_rgb: [255,255,255], // Monochrome until the user selects another LED color.
            led_intensity: 0.72,
            mirror_opacity: 0.85,
            reduce_motion: false,
        }
    }
}
impl NoirSettings {
    fn path() -> PathBuf {
        let home = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        home.join("GEKKO").join("Native").join("appearance.json")
    }
    pub fn load() -> Self {
        fs::read_to_string(Self::path()).ok()
            .and_then(|s| serde_json::from_str::<NoirSettings>(&s).ok())
            .unwrap_or_default().validated()
    }
    fn validated(mut self) -> Self {
        if !self.led_intensity.is_finite() { self.led_intensity=0.72; }
        if !self.mirror_opacity.is_finite() { self.mirror_opacity=0.85; }
        self.led_intensity=self.led_intensity.clamp(0.0,1.0);
        self.mirror_opacity=self.mirror_opacity.clamp(0.55,0.98);
        self
    }
    pub fn save(&self) -> std::io::Result<()> {
        let path=Self::path();
        if let Some(parent)=path.parent(){ fs::create_dir_all(parent)?; }
        let json=serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let temp=path.with_extension("json.tmp");
        fs::write(&temp,json)?;
        if fs::rename(&temp,&path).is_err() {
            fs::copy(&temp,&path)?;
            let _=fs::remove_file(temp);
        }
        Ok(())
    }
    pub fn led(&self) -> egui::Color32 {
        egui::Color32::from_rgb(self.led_rgb[0],self.led_rgb[1],self.led_rgb[2])
    }
}

fn glow_color(rgb: [u8;3],alpha:f32) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(rgb[0],rgb[1],rgb[2],alpha.clamp(0.0,255.0) as u8)
}

/// Layered translucent specular bands create a dark mirror effect.
/// This is a *painted* reflection, not Chromium CSS or a claim of OS desktop blur.
pub fn paint_mirror(ui: &egui::Ui, settings: &NoirSettings) {
    let r=ui.max_rect();
    let p=ui.painter();
    // Opaque-safe black body, with adjustable dark-mirror overcoat.
    let shade=(6.0+(1.0-settings.mirror_opacity)*17.0).round() as u8;
    p.rect_filled(r,0.0,egui::Color32::from_gray(shade));
    let header=egui::Rect::from_min_max(
        r.min+egui::vec2(2.0,2.0),egui::pos2(r.max.x-2.0,r.min.y+22.0));
    let band_alpha=(settings.mirror_opacity*29.0) as u8;
    p.rect_filled(header,egui::CornerRadius::same(14),
        egui::Color32::from_white_alpha(band_alpha));
    p.line_segment([
        egui::pos2(r.min.x+10.0,r.min.y+2.0),
        egui::pos2(r.max.x-10.0,r.min.y+2.0)],
        egui::Stroke::new(1.0,egui::Color32::from_white_alpha(45)));
    p.line_segment([
        egui::pos2(r.min.x+9.0,r.max.y-2.0),
        egui::pos2(r.max.x-9.0,r.max.y-2.0)],
        egui::Stroke::new(1.0,egui::Color32::from_white_alpha(29)));
}

/// Flat black control with a customizable LED *around* the hit area, never a tinted background.
pub fn led_button(ui: &mut egui::Ui,label: impl Into<String>,size: egui::Vec2,
    settings:&NoirSettings,selected:bool,enabled:bool) -> egui::Response {
    let text=egui::RichText::new(label.into()).size(16.0).color(egui::Color32::WHITE);
    let button=egui::Button::new(text)
        .min_size(size).corner_radius(egui::CornerRadius::same(11))
        .fill(if selected {egui::Color32::from_gray(43)} else {egui::Color32::from_gray(18)})
        .stroke(egui::Stroke::new(1.0,
            if selected {egui::Color32::from_gray(195)} else {egui::Color32::from_gray(59)}));
    let response=ui.add_enabled(enabled,button);
    let on=enabled && (response.hovered() || response.has_focus());
    let glow=if settings.reduce_motion {if on {1.0} else {0.0}} else {
        ui.ctx().animate_bool_with_time(response.id.with("led"),on,0.16)
    };
    let power=glow*settings.led_intensity;
    if power>0.001 {
        let painter=ui.painter();
        let outer=response.rect.expand(1.0);
        // Glow is drawn around the button; the background remains gray-black.
        painter.rect_stroke(outer.expand(3.0),egui::CornerRadius::same(14),
            egui::Stroke::new(5.0,glow_color(settings.led_rgb,power*17.0)),
            egui::StrokeKind::Outside);
        painter.rect_stroke(outer.expand(1.5),egui::CornerRadius::same(12),
            egui::Stroke::new(3.0,glow_color(settings.led_rgb,power*43.0)),
            egui::StrokeKind::Outside);
        painter.rect_stroke(outer,egui::CornerRadius::same(11),
            egui::Stroke::new(1.5,glow_color(settings.led_rgb,power*230.0)),
            egui::StrokeKind::Outside);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn default_theme_is_monochrome() {
        let s=NoirSettings::default();
        assert_eq!(s.led_rgb,[255,255,255]);
        assert_eq!(s.led(),egui::Color32::WHITE);
    }
    #[test] fn settings_roundtrip_and_clamp() {
        let s=NoirSettings {led_rgb:[255,20,205],led_intensity:8.0,
            mirror_opacity: -2.0,reduce_motion:true};
        let encoded=serde_json::to_string(&s).unwrap();
        let parsed:NoirSettings=serde_json::from_str(&encoded).unwrap();
        let safe=parsed.validated();
        assert_eq!(safe.led_rgb,[255,20,205]);
        assert_eq!(safe.led_intensity,1.0);
        assert_eq!(safe.mirror_opacity,0.55);
        assert!(safe.reduce_motion);
    }
    #[test] fn incomplete_preferences_use_defaults() {
        let s:NoirSettings=serde_json::from_str(r#"{"led_rgb":[100,90,200]}"#).unwrap();
        assert_eq!(s.mirror_opacity,0.85);
        assert!(!s.reduce_motion);
    }
}
