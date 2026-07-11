//! Per-state status bar icons based on the SF Symbol coffee cup.
//!
//! The symbol `cup.and.saucer.fill` provides the nice, system-native coffee
//! glyph. To make the color feedback (gray/yellow/orange) actually visible in
//! the menu bar, we apply the color via an `NSImageSymbolConfiguration`
//! (`configurationWithHierarchicalColor:`) and re-render the symbol with
//! `imageWithSymbolConfiguration:`.
//!
//! Why not `contentTintColor:` on the button or `withTintColor:` on the image?
//! - The button's `contentTintColor:` was unreliable and left the icon black.
//! - `NSImage.withTintColor:` is an unrecognized selector on macOS 26 (Tahoe);
//!   Apple removed it in favor of symbol configurations.
//!
//! The hierarchical-color configuration tints the symbol's layers from the
//! given color, which is exactly the visible, deterministic result we want.

use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSImage, NSImageSymbolConfiguration};
use objc2_foundation::NSString;

use crate::state::Mode;

/// The SF Symbol name used for every state.
const SYMBOL: &str = "cup.and.saucer.fill";

/// The color for a given mode.
pub fn color_for_mode(mode: Mode) -> Retained<NSColor> {
    match mode {
        // A muted gray that reads as "inactive" in the menu bar.
        Mode::Off => NSColor::colorWithCalibratedWhite_alpha(0.45, 1.0),
        // Warm yellow for "idle-only" prevention.
        Mode::IdleOnly => NSColor::colorWithSRGBRed_green_blue_alpha(0.95, 0.69, 0.13, 1.0),
        // Heavier, more saturated orange for "idle + display" — reads as a hot,
        // "fully on" state and is clearly distinct from the yellow above.
        Mode::IdleAndDisplay => NSColor::colorWithSRGBRed_green_blue_alpha(0.86, 0.34, 0.09, 1.0),
    }
}

/// Build the status bar icon for a mode: the coffee symbol tinted with the
/// mode color via a hierarchical symbol configuration. Returns `None` only if
/// the SF Symbol is unavailable.
pub fn image_for_mode(mode: Mode) -> Option<Retained<NSImage>> {
    let name = NSString::from_str(SYMBOL);
    let base = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &name,
        Some(&NSString::from_str("Cafe keep-awake indicator")),
    )?;

    let color = color_for_mode(mode);
    let config = NSImageSymbolConfiguration::configurationWithHierarchicalColor(&color);
    // Returns a new symbol image rendered with the configuration's color, or
    // `None` if the configuration cannot be applied (in which case we fall
    // back to the base symbol — still a valid, displayable image).
    Some(base.imageWithSymbolConfiguration(&config).unwrap_or(base))
}
