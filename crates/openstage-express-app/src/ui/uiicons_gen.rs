// SPDX-License-Identifier: Apache-2.0 AND ISC
// Made by tools/make_ui_icons.py in the OpenStage Author repository, from its design/icons.json (the suite's one list of icons):
// do not edit by hand. Change that list and run the tool there with --express pointing at this file.
//! The icons of Capture Express's own buttons, menus and panes: Lucide 1.56.0, each kept as the lines it is drawn with on its 24 by 24 grid
//! (curves and arcs cut into short straight pieces, none more than 0.08 of a grid square off the curve). `icon.rs` draws them
//! 2 grid squares thick in the colour of the words beside them. An icon says nothing to a screen reader: the words beside it, or
//! the name of its button, say what it is.
//
// ISC License
//
// Copyright (c) 2026 Lucide Icons and Contributors
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//
// ---
//
// The following Lucide icons are derived from the Feather project:
//
// airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out
//
// The MIT License (MIT) (for the icons listed above)
//
// Copyright (c) 2013-present Cole Bemis
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// One line of an icon: points on the 24 by 24 grid, joined in order, and whether the last is joined back to the first.
pub struct IconLine {
    pub points: &'static [[f32; 2]],
    pub closed: bool,
}

/// Every icon the program uses, by its Lucide name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiIcon {
    /// Lucide `app-window`.
    AppWindow,
    /// Lucide `archive`.
    Archive,
    /// Lucide `archive-restore`.
    ArchiveRestore,
    /// Lucide `arrow-down`.
    ArrowDown,
    /// Lucide `arrow-left`.
    ArrowLeft,
    /// Lucide `arrow-up`.
    ArrowUp,
    /// Lucide `arrow-up-down`.
    ArrowUpDown,
    /// Lucide `book-open`.
    BookOpen,
    /// Lucide `camera`.
    Camera,
    /// Lucide `check`.
    Check,
    /// Lucide `chevron-down`.
    ChevronDown,
    /// Lucide `chevron-up`.
    ChevronUp,
    /// Lucide `circle`.
    Circle,
    /// Lucide `circle-check`.
    CircleCheck,
    /// Lucide `circle-dot`.
    CircleDot,
    /// Lucide `circle-stop`.
    CircleStop,
    /// Lucide `copy-plus`.
    CopyPlus,
    /// Lucide `crop`.
    Crop,
    /// Lucide `droplets`.
    Droplets,
    /// Lucide `ellipsis`.
    Ellipsis,
    /// Lucide `eye-off`.
    EyeOff,
    /// Lucide `file-output`.
    FileOutput,
    /// Lucide `file-text`.
    FileText,
    /// Lucide `flashlight`.
    Flashlight,
    /// Lucide `focus`.
    Focus,
    /// Lucide `folder-open`.
    FolderOpen,
    /// Lucide `hash`.
    Hash,
    /// Lucide `highlighter`.
    Highlighter,
    /// Lucide `info`.
    Info,
    /// Lucide `keyboard`.
    Keyboard,
    /// Lucide `list-checks`.
    ListChecks,
    /// Lucide `minus`.
    Minus,
    /// Lucide `monitor`.
    Monitor,
    /// Lucide `moon`.
    Moon,
    /// Lucide `move-up-right`.
    MoveUpRight,
    /// Lucide `package`.
    Package,
    /// Lucide `package-open`.
    PackageOpen,
    /// Lucide `pause`.
    Pause,
    /// Lucide `pen-tool`.
    PenTool,
    /// Lucide `pencil`.
    Pencil,
    /// Lucide `play`.
    Play,
    /// Lucide `plus`.
    Plus,
    /// Lucide `redo-2`.
    Redo2,
    /// Lucide `scan`.
    Scan,
    /// Lucide `search`.
    Search,
    /// Lucide `settings`.
    Settings,
    /// Lucide `shapes`.
    Shapes,
    /// Lucide `square`.
    Square,
    /// Lucide `square-dashed`.
    SquareDashed,
    /// Lucide `sun`.
    Sun,
    /// Lucide `trash`.
    Trash,
    /// Lucide `type`.
    Type,
    /// Lucide `undo-2`.
    Undo2,
    /// Lucide `x`.
    X,
    /// Lucide `zoom-in`.
    ZoomIn,
    /// Lucide `zoom-out`.
    ZoomOut,
}

impl UiIcon {
    pub const ALL: [UiIcon; 56] = [
        UiIcon::AppWindow,
        UiIcon::Archive,
        UiIcon::ArchiveRestore,
        UiIcon::ArrowDown,
        UiIcon::ArrowLeft,
        UiIcon::ArrowUp,
        UiIcon::ArrowUpDown,
        UiIcon::BookOpen,
        UiIcon::Camera,
        UiIcon::Check,
        UiIcon::ChevronDown,
        UiIcon::ChevronUp,
        UiIcon::Circle,
        UiIcon::CircleCheck,
        UiIcon::CircleDot,
        UiIcon::CircleStop,
        UiIcon::CopyPlus,
        UiIcon::Crop,
        UiIcon::Droplets,
        UiIcon::Ellipsis,
        UiIcon::EyeOff,
        UiIcon::FileOutput,
        UiIcon::FileText,
        UiIcon::Flashlight,
        UiIcon::Focus,
        UiIcon::FolderOpen,
        UiIcon::Hash,
        UiIcon::Highlighter,
        UiIcon::Info,
        UiIcon::Keyboard,
        UiIcon::ListChecks,
        UiIcon::Minus,
        UiIcon::Monitor,
        UiIcon::Moon,
        UiIcon::MoveUpRight,
        UiIcon::Package,
        UiIcon::PackageOpen,
        UiIcon::Pause,
        UiIcon::PenTool,
        UiIcon::Pencil,
        UiIcon::Play,
        UiIcon::Plus,
        UiIcon::Redo2,
        UiIcon::Scan,
        UiIcon::Search,
        UiIcon::Settings,
        UiIcon::Shapes,
        UiIcon::Square,
        UiIcon::SquareDashed,
        UiIcon::Sun,
        UiIcon::Trash,
        UiIcon::Type,
        UiIcon::Undo2,
        UiIcon::X,
        UiIcon::ZoomIn,
        UiIcon::ZoomOut,
    ];

    /// Its Lucide name, as on lucide.dev.
    pub fn name(self) -> &'static str {
        match self {
            UiIcon::AppWindow => "app-window",
            UiIcon::Archive => "archive",
            UiIcon::ArchiveRestore => "archive-restore",
            UiIcon::ArrowDown => "arrow-down",
            UiIcon::ArrowLeft => "arrow-left",
            UiIcon::ArrowUp => "arrow-up",
            UiIcon::ArrowUpDown => "arrow-up-down",
            UiIcon::BookOpen => "book-open",
            UiIcon::Camera => "camera",
            UiIcon::Check => "check",
            UiIcon::ChevronDown => "chevron-down",
            UiIcon::ChevronUp => "chevron-up",
            UiIcon::Circle => "circle",
            UiIcon::CircleCheck => "circle-check",
            UiIcon::CircleDot => "circle-dot",
            UiIcon::CircleStop => "circle-stop",
            UiIcon::CopyPlus => "copy-plus",
            UiIcon::Crop => "crop",
            UiIcon::Droplets => "droplets",
            UiIcon::Ellipsis => "ellipsis",
            UiIcon::EyeOff => "eye-off",
            UiIcon::FileOutput => "file-output",
            UiIcon::FileText => "file-text",
            UiIcon::Flashlight => "flashlight",
            UiIcon::Focus => "focus",
            UiIcon::FolderOpen => "folder-open",
            UiIcon::Hash => "hash",
            UiIcon::Highlighter => "highlighter",
            UiIcon::Info => "info",
            UiIcon::Keyboard => "keyboard",
            UiIcon::ListChecks => "list-checks",
            UiIcon::Minus => "minus",
            UiIcon::Monitor => "monitor",
            UiIcon::Moon => "moon",
            UiIcon::MoveUpRight => "move-up-right",
            UiIcon::Package => "package",
            UiIcon::PackageOpen => "package-open",
            UiIcon::Pause => "pause",
            UiIcon::PenTool => "pen-tool",
            UiIcon::Pencil => "pencil",
            UiIcon::Play => "play",
            UiIcon::Plus => "plus",
            UiIcon::Redo2 => "redo-2",
            UiIcon::Scan => "scan",
            UiIcon::Search => "search",
            UiIcon::Settings => "settings",
            UiIcon::Shapes => "shapes",
            UiIcon::Square => "square",
            UiIcon::SquareDashed => "square-dashed",
            UiIcon::Sun => "sun",
            UiIcon::Trash => "trash",
            UiIcon::Type => "type",
            UiIcon::Undo2 => "undo-2",
            UiIcon::X => "x",
            UiIcon::ZoomIn => "zoom-in",
            UiIcon::ZoomOut => "zoom-out",
        }
    }

    /// The lines it is drawn with.
    pub fn lines(self) -> &'static [IconLine] {
        match self {
            UiIcon::AppWindow => &APPWINDOW_LINES,
            UiIcon::Archive => &ARCHIVE_LINES,
            UiIcon::ArchiveRestore => &ARCHIVERESTORE_LINES,
            UiIcon::ArrowDown => &ARROWDOWN_LINES,
            UiIcon::ArrowLeft => &ARROWLEFT_LINES,
            UiIcon::ArrowUp => &ARROWUP_LINES,
            UiIcon::ArrowUpDown => &ARROWUPDOWN_LINES,
            UiIcon::BookOpen => &BOOKOPEN_LINES,
            UiIcon::Camera => &CAMERA_LINES,
            UiIcon::Check => &CHECK_LINES,
            UiIcon::ChevronDown => &CHEVRONDOWN_LINES,
            UiIcon::ChevronUp => &CHEVRONUP_LINES,
            UiIcon::Circle => &CIRCLE_LINES,
            UiIcon::CircleCheck => &CIRCLECHECK_LINES,
            UiIcon::CircleDot => &CIRCLEDOT_LINES,
            UiIcon::CircleStop => &CIRCLESTOP_LINES,
            UiIcon::CopyPlus => &COPYPLUS_LINES,
            UiIcon::Crop => &CROP_LINES,
            UiIcon::Droplets => &DROPLETS_LINES,
            UiIcon::Ellipsis => &ELLIPSIS_LINES,
            UiIcon::EyeOff => &EYEOFF_LINES,
            UiIcon::FileOutput => &FILEOUTPUT_LINES,
            UiIcon::FileText => &FILETEXT_LINES,
            UiIcon::Flashlight => &FLASHLIGHT_LINES,
            UiIcon::Focus => &FOCUS_LINES,
            UiIcon::FolderOpen => &FOLDEROPEN_LINES,
            UiIcon::Hash => &HASH_LINES,
            UiIcon::Highlighter => &HIGHLIGHTER_LINES,
            UiIcon::Info => &INFO_LINES,
            UiIcon::Keyboard => &KEYBOARD_LINES,
            UiIcon::ListChecks => &LISTCHECKS_LINES,
            UiIcon::Minus => &MINUS_LINES,
            UiIcon::Monitor => &MONITOR_LINES,
            UiIcon::Moon => &MOON_LINES,
            UiIcon::MoveUpRight => &MOVEUPRIGHT_LINES,
            UiIcon::Package => &PACKAGE_LINES,
            UiIcon::PackageOpen => &PACKAGEOPEN_LINES,
            UiIcon::Pause => &PAUSE_LINES,
            UiIcon::PenTool => &PENTOOL_LINES,
            UiIcon::Pencil => &PENCIL_LINES,
            UiIcon::Play => &PLAY_LINES,
            UiIcon::Plus => &PLUS_LINES,
            UiIcon::Redo2 => &REDO2_LINES,
            UiIcon::Scan => &SCAN_LINES,
            UiIcon::Search => &SEARCH_LINES,
            UiIcon::Settings => &SETTINGS_LINES,
            UiIcon::Shapes => &SHAPES_LINES,
            UiIcon::Square => &SQUARE_LINES,
            UiIcon::SquareDashed => &SQUAREDASHED_LINES,
            UiIcon::Sun => &SUN_LINES,
            UiIcon::Trash => &TRASH_LINES,
            UiIcon::Type => &TYPE_LINES,
            UiIcon::Undo2 => &UNDO2_LINES,
            UiIcon::X => &X_LINES,
            UiIcon::ZoomIn => &ZOOMIN_LINES,
            UiIcon::ZoomOut => &ZOOMOUT_LINES,
        }
    }
}

const APPWINDOW_LINES: [IconLine; 4] = [
    IconLine { points: &[[20.0, 4.0], [21.0, 4.27], [21.73, 5.0], [22.0, 6.0], [22.0, 18.0], [21.73, 19.0], [21.0, 19.73], [20.0, 20.0], [4.0, 20.0], [3.0, 19.73], [2.27, 19.0], [2.0, 18.0], [2.0, 6.0], [2.27, 5.0], [3.0, 4.27], [4.0, 4.0]], closed: true },
    IconLine { points: &[[10.0, 4.0], [10.0, 8.0]], closed: false },
    IconLine { points: &[[2.0, 8.0], [22.0, 8.0]], closed: false },
    IconLine { points: &[[6.0, 4.0], [6.0, 8.0]], closed: false },
];
const ARCHIVE_LINES: [IconLine; 3] = [
    IconLine { points: &[[21.0, 3.0], [21.71, 3.29], [22.0, 4.0], [22.0, 7.0], [21.71, 7.71], [21.0, 8.0], [3.0, 8.0], [2.29, 7.71], [2.0, 7.0], [2.0, 4.0], [2.29, 3.29], [3.0, 3.0]], closed: true },
    IconLine { points: &[[4.0, 8.0], [4.0, 19.0], [4.27, 20.0], [5.0, 20.73], [6.0, 21.0], [18.0, 21.0], [19.0, 20.73], [19.73, 20.0], [20.0, 19.0], [20.0, 8.0]], closed: false },
    IconLine { points: &[[10.0, 12.0], [14.0, 12.0]], closed: false },
];
const ARCHIVERESTORE_LINES: [IconLine; 5] = [
    IconLine { points: &[[21.0, 3.0], [21.71, 3.29], [22.0, 4.0], [22.0, 7.0], [21.71, 7.71], [21.0, 8.0], [3.0, 8.0], [2.29, 7.71], [2.0, 7.0], [2.0, 4.0], [2.29, 3.29], [3.0, 3.0]], closed: true },
    IconLine { points: &[[4.0, 8.0], [4.0, 19.0], [4.27, 20.0], [5.0, 20.73], [6.0, 21.0], [8.0, 21.0]], closed: false },
    IconLine { points: &[[20.0, 8.0], [20.0, 19.0], [19.73, 20.0], [19.0, 20.73], [18.0, 21.0], [16.0, 21.0]], closed: false },
    IconLine { points: &[[9.0, 15.0], [12.0, 12.0], [15.0, 15.0]], closed: false },
    IconLine { points: &[[12.0, 12.0], [12.0, 21.0]], closed: false },
];
const ARROWDOWN_LINES: [IconLine; 2] = [
    IconLine { points: &[[12.0, 5.0], [12.0, 19.0]], closed: false },
    IconLine { points: &[[19.0, 12.0], [12.0, 19.0], [5.0, 12.0]], closed: false },
];
const ARROWLEFT_LINES: [IconLine; 2] = [
    IconLine { points: &[[12.0, 19.0], [5.0, 12.0], [12.0, 5.0]], closed: false },
    IconLine { points: &[[19.0, 12.0], [5.0, 12.0]], closed: false },
];
const ARROWUP_LINES: [IconLine; 2] = [
    IconLine { points: &[[5.0, 12.0], [12.0, 5.0], [19.0, 12.0]], closed: false },
    IconLine { points: &[[12.0, 19.0], [12.0, 5.0]], closed: false },
];
const ARROWUPDOWN_LINES: [IconLine; 4] = [
    IconLine { points: &[[21.0, 16.0], [17.0, 20.0], [13.0, 16.0]], closed: false },
    IconLine { points: &[[17.0, 20.0], [17.0, 4.0]], closed: false },
    IconLine { points: &[[3.0, 8.0], [7.0, 4.0], [11.0, 8.0]], closed: false },
    IconLine { points: &[[7.0, 4.0], [7.0, 20.0]], closed: false },
];
const BOOKOPEN_LINES: [IconLine; 2] = [
    IconLine { points: &[[12.0, 5.0], [12.0, 21.0]], closed: false },
    IconLine { points: &[[20.0, 19.0], [21.0, 18.73], [21.73, 18.0], [22.0, 17.0], [22.0, 5.0], [21.73, 4.0], [21.0, 3.27], [20.0, 3.0], [16.0, 3.0], [14.48, 3.24], [13.1, 3.93], [12.0, 5.0], [10.9, 3.93], [9.52, 3.24], [8.0, 3.0], [4.0, 3.0], [3.0, 3.27], [2.27, 4.0], [2.0, 5.0], [2.0, 17.0], [2.27, 18.0], [3.0, 18.73], [4.0, 19.0], [8.0, 19.0], [9.52, 19.24], [10.9, 19.93], [12.0, 21.0], [13.1, 19.93], [14.48, 19.24], [16.0, 19.0]], closed: true },
];
const CAMERA_LINES: [IconLine; 2] = [
    IconLine { points: &[[14.0, 4.0], [15.02, 4.28], [15.76, 5.05], [16.24, 5.95], [16.98, 6.72], [18.0, 7.0], [20.0, 7.0], [21.0, 7.27], [21.73, 8.0], [22.0, 9.0], [22.0, 18.0], [21.73, 19.0], [21.0, 19.73], [20.0, 20.0], [4.0, 20.0], [3.0, 19.73], [2.27, 19.0], [2.0, 18.0], [2.0, 9.0], [2.27, 8.0], [3.0, 7.27], [4.0, 7.0], [6.0, 7.0], [7.02, 6.72], [7.76, 5.95], [8.25, 5.05], [8.98, 4.28], [10.0, 4.0]], closed: true },
    IconLine { points: &[[15.0, 13.0], [14.7, 14.3], [13.87, 15.35], [12.67, 15.92], [11.33, 15.92], [10.13, 15.35], [9.3, 14.3], [9.0, 13.0], [9.3, 11.7], [10.13, 10.65], [11.33, 10.08], [12.67, 10.08], [13.87, 10.65], [14.7, 11.7]], closed: true },
];
const CHECK_LINES: [IconLine; 1] = [
    IconLine { points: &[[20.0, 6.0], [9.0, 17.0], [4.0, 12.0]], closed: false },
];
const CHEVRONDOWN_LINES: [IconLine; 1] = [
    IconLine { points: &[[6.0, 9.0], [12.0, 15.0], [18.0, 9.0]], closed: false },
];
const CHEVRONUP_LINES: [IconLine; 1] = [
    IconLine { points: &[[18.0, 15.0], [12.0, 9.0], [6.0, 15.0]], closed: false },
];
const CIRCLE_LINES: [IconLine; 1] = [
    IconLine { points: &[[22.0, 12.0], [21.69, 14.49], [20.76, 16.82], [19.29, 18.85], [17.36, 20.44], [15.09, 21.51], [12.63, 21.98], [10.13, 21.82], [7.74, 21.05], [5.63, 19.71], [3.91, 17.88], [2.7, 15.68], [2.08, 13.25], [2.08, 10.75], [2.7, 8.32], [3.91, 6.12], [5.63, 4.29], [7.74, 2.95], [10.13, 2.18], [12.63, 2.02], [15.09, 2.49], [17.36, 3.56], [19.29, 5.15], [20.76, 7.18], [21.69, 9.51]], closed: true },
];
const CIRCLECHECK_LINES: [IconLine; 2] = [
    IconLine { points: &[[22.0, 12.0], [21.69, 14.49], [20.76, 16.82], [19.29, 18.85], [17.36, 20.44], [15.09, 21.51], [12.63, 21.98], [10.13, 21.82], [7.74, 21.05], [5.63, 19.71], [3.91, 17.88], [2.7, 15.68], [2.08, 13.25], [2.08, 10.75], [2.7, 8.32], [3.91, 6.12], [5.63, 4.29], [7.74, 2.95], [10.13, 2.18], [12.63, 2.02], [15.09, 2.49], [17.36, 3.56], [19.29, 5.15], [20.76, 7.18], [21.69, 9.51]], closed: true },
    IconLine { points: &[[16.0, 9.0], [10.5, 14.5], [8.0, 12.0]], closed: false },
];
const CIRCLEDOT_LINES: [IconLine; 2] = [
    IconLine { points: &[[13.0, 12.0], [12.87, 12.5], [12.5, 12.87], [12.0, 13.0], [11.5, 12.87], [11.13, 12.5], [11.0, 12.0], [11.13, 11.5], [11.5, 11.13], [12.0, 11.0], [12.5, 11.13], [12.87, 11.5]], closed: true },
    IconLine { points: &[[22.0, 12.0], [21.69, 14.49], [20.76, 16.82], [19.29, 18.85], [17.36, 20.44], [15.09, 21.51], [12.63, 21.98], [10.13, 21.82], [7.74, 21.05], [5.63, 19.71], [3.91, 17.88], [2.7, 15.68], [2.08, 13.25], [2.08, 10.75], [2.7, 8.32], [3.91, 6.12], [5.63, 4.29], [7.74, 2.95], [10.13, 2.18], [12.63, 2.02], [15.09, 2.49], [17.36, 3.56], [19.29, 5.15], [20.76, 7.18], [21.69, 9.51]], closed: true },
];
const CIRCLESTOP_LINES: [IconLine; 2] = [
    IconLine { points: &[[22.0, 12.0], [21.69, 14.49], [20.76, 16.82], [19.29, 18.85], [17.36, 20.44], [15.09, 21.51], [12.63, 21.98], [10.13, 21.82], [7.74, 21.05], [5.63, 19.71], [3.91, 17.88], [2.7, 15.68], [2.08, 13.25], [2.08, 10.75], [2.7, 8.32], [3.91, 6.12], [5.63, 4.29], [7.74, 2.95], [10.13, 2.18], [12.63, 2.02], [15.09, 2.49], [17.36, 3.56], [19.29, 5.15], [20.76, 7.18], [21.69, 9.51]], closed: true },
    IconLine { points: &[[14.0, 9.0], [14.71, 9.29], [15.0, 10.0], [15.0, 14.0], [14.71, 14.71], [14.0, 15.0], [10.0, 15.0], [9.29, 14.71], [9.0, 14.0], [9.0, 10.0], [9.29, 9.29], [10.0, 9.0]], closed: true },
];
const COPYPLUS_LINES: [IconLine; 4] = [
    IconLine { points: &[[15.0, 12.0], [15.0, 18.0]], closed: false },
    IconLine { points: &[[12.0, 15.0], [18.0, 15.0]], closed: false },
    IconLine { points: &[[20.0, 8.0], [21.0, 8.27], [21.73, 9.0], [22.0, 10.0], [22.0, 20.0], [21.73, 21.0], [21.0, 21.73], [20.0, 22.0], [10.0, 22.0], [9.0, 21.73], [8.27, 21.0], [8.0, 20.0], [8.0, 10.0], [8.27, 9.0], [9.0, 8.27], [10.0, 8.0]], closed: true },
    IconLine { points: &[[4.0, 16.0], [2.99, 15.73], [2.27, 15.01], [2.0, 14.0], [2.0, 4.0], [2.27, 2.99], [2.99, 2.27], [4.0, 2.0], [14.0, 2.0], [15.01, 2.27], [15.73, 2.99], [16.0, 4.0]], closed: false },
];
const CROP_LINES: [IconLine; 2] = [
    IconLine { points: &[[6.0, 2.0], [6.0, 16.0], [6.27, 17.0], [7.0, 17.73], [8.0, 18.0], [22.0, 18.0]], closed: false },
    IconLine { points: &[[18.0, 22.0], [18.0, 8.0], [17.73, 7.0], [17.0, 6.27], [16.0, 6.0], [2.0, 6.0]], closed: false },
];
const DROPLETS_LINES: [IconLine; 2] = [
    IconLine { points: &[[7.0, 16.3], [8.26, 16.09], [9.36, 15.51], [10.23, 14.63], [10.8, 13.52], [11.0, 12.25], [10.81, 11.11], [10.24, 10.04], [9.29, 9.06], [8.25, 7.99], [7.47, 6.71], [7.0, 5.3], [6.53, 6.71], [5.75, 8.0], [4.71, 9.06], [3.76, 10.04], [3.19, 11.12], [3.0, 12.25], [3.2, 13.52], [3.77, 14.63], [4.64, 15.51], [5.74, 16.09]], closed: true },
    IconLine { points: &[[12.56, 6.6], [13.44, 4.87], [14.0, 3.02], [14.55, 4.87], [15.44, 6.61], [16.6, 8.18], [18.0, 9.52], [19.31, 10.77], [20.25, 12.12], [20.81, 13.54], [21.0, 15.02], [20.71, 17.05], [19.84, 18.91], [18.46, 20.43], [16.71, 21.49], [14.72, 21.99], [12.67, 21.9], [10.74, 21.21], [9.09, 19.99]], closed: false },
];
const ELLIPSIS_LINES: [IconLine; 3] = [
    IconLine { points: &[[13.0, 12.0], [12.87, 12.5], [12.5, 12.87], [12.0, 13.0], [11.5, 12.87], [11.13, 12.5], [11.0, 12.0], [11.13, 11.5], [11.5, 11.13], [12.0, 11.0], [12.5, 11.13], [12.87, 11.5]], closed: true },
    IconLine { points: &[[20.0, 12.0], [19.87, 12.5], [19.5, 12.87], [19.0, 13.0], [18.5, 12.87], [18.13, 12.5], [18.0, 12.0], [18.13, 11.5], [18.5, 11.13], [19.0, 11.0], [19.5, 11.13], [19.87, 11.5]], closed: true },
    IconLine { points: &[[6.0, 12.0], [5.87, 12.5], [5.5, 12.87], [5.0, 13.0], [4.5, 12.87], [4.13, 12.5], [4.0, 12.0], [4.13, 11.5], [4.5, 11.13], [5.0, 11.0], [5.5, 11.13], [5.87, 11.5]], closed: true },
];
const EYEOFF_LINES: [IconLine; 4] = [
    IconLine { points: &[[10.73, 5.08], [13.05, 5.05], [15.33, 5.53], [17.44, 6.48], [19.3, 7.86], [20.83, 9.61], [21.94, 11.65], [22.0, 12.0], [21.94, 12.35], [21.3, 13.64], [20.49, 14.84]], closed: false },
    IconLine { points: &[[14.08, 14.16], [12.96, 14.84], [11.64, 14.98], [10.4, 14.54], [9.46, 13.6], [9.02, 12.36], [9.16, 11.04], [9.84, 9.92]], closed: false },
    IconLine { points: &[[17.48, 17.5], [15.39, 18.45], [13.14, 18.94], [10.84, 18.94], [8.59, 18.45], [6.5, 17.49], [4.66, 16.11], [3.16, 14.37], [2.06, 12.35], [2.0, 12.0], [2.06, 11.65], [3.16, 9.63], [4.67, 7.89], [6.51, 6.51]], closed: false },
    IconLine { points: &[[2.0, 2.0], [22.0, 22.0]], closed: false },
];
const FILEOUTPUT_LINES: [IconLine; 4] = [
    IconLine { points: &[[4.23, 20.93], [4.96, 21.71], [6.0, 22.0], [18.0, 22.0], [19.0, 21.73], [19.73, 21.0], [20.0, 20.0], [20.0, 8.0], [19.82, 7.08], [19.29, 6.29], [15.71, 2.71], [14.92, 2.18], [14.0, 2.0], [6.0, 2.0], [5.0, 2.27], [4.27, 3.0], [4.0, 4.0], [4.0, 7.13]], closed: false },
    IconLine { points: &[[14.0, 2.0], [14.0, 7.0], [14.29, 7.71], [15.0, 8.0], [20.0, 8.0]], closed: false },
    IconLine { points: &[[5.0, 11.0], [2.0, 14.0]], closed: false },
    IconLine { points: &[[5.0, 17.0], [2.0, 14.0], [12.0, 14.0]], closed: false },
];
const FILETEXT_LINES: [IconLine; 5] = [
    IconLine { points: &[[6.0, 22.0], [5.0, 21.73], [4.27, 21.0], [4.0, 20.0], [4.0, 4.0], [4.27, 3.0], [5.0, 2.27], [6.0, 2.0], [14.0, 2.0], [14.92, 2.18], [15.7, 2.71], [19.29, 6.29], [19.82, 7.08], [20.0, 8.0], [20.0, 20.0], [19.73, 21.0], [19.0, 21.73], [18.0, 22.0]], closed: true },
    IconLine { points: &[[14.0, 2.0], [14.0, 7.0], [14.29, 7.71], [15.0, 8.0], [20.0, 8.0]], closed: false },
    IconLine { points: &[[10.0, 9.0], [8.0, 9.0]], closed: false },
    IconLine { points: &[[16.0, 13.0], [8.0, 13.0]], closed: false },
    IconLine { points: &[[16.0, 17.0], [8.0, 17.0]], closed: false },
];
const FLASHLIGHT_LINES: [IconLine; 3] = [
    IconLine { points: &[[12.0, 13.0], [12.0, 14.0]], closed: false },
    IconLine { points: &[[17.0, 2.0], [17.71, 2.29], [18.0, 3.0], [18.0, 7.0], [17.85, 7.95], [17.4, 8.8], [16.8, 9.6], [16.21, 10.74], [16.0, 12.0], [16.0, 20.0], [15.73, 21.0], [15.0, 21.73], [14.0, 22.0], [10.0, 22.0], [9.0, 21.73], [8.27, 21.0], [8.0, 20.0], [8.0, 12.0], [7.79, 10.74], [7.2, 9.6], [6.6, 8.8], [6.15, 7.95], [6.0, 7.0], [6.0, 3.0], [6.29, 2.29], [7.0, 2.0]], closed: true },
    IconLine { points: &[[6.0, 6.0], [18.0, 6.0]], closed: false },
];
const FOCUS_LINES: [IconLine; 5] = [
    IconLine { points: &[[15.0, 12.0], [14.7, 13.3], [13.87, 14.35], [12.67, 14.92], [11.33, 14.92], [10.13, 14.35], [9.3, 13.3], [9.0, 12.0], [9.3, 10.7], [10.13, 9.65], [11.33, 9.08], [12.67, 9.08], [13.87, 9.65], [14.7, 10.7]], closed: true },
    IconLine { points: &[[3.0, 7.0], [3.0, 5.0], [3.27, 4.0], [4.0, 3.27], [5.0, 3.0], [7.0, 3.0]], closed: false },
    IconLine { points: &[[17.0, 3.0], [19.0, 3.0], [20.0, 3.27], [20.73, 4.0], [21.0, 5.0], [21.0, 7.0]], closed: false },
    IconLine { points: &[[21.0, 17.0], [21.0, 19.0], [20.73, 20.0], [20.0, 20.73], [19.0, 21.0], [17.0, 21.0]], closed: false },
    IconLine { points: &[[7.0, 21.0], [5.0, 21.0], [4.0, 20.73], [3.27, 20.0], [3.0, 19.0], [3.0, 17.0]], closed: false },
];
const FOLDEROPEN_LINES: [IconLine; 1] = [
    IconLine { points: &[[6.0, 14.0], [7.5, 11.1], [8.22, 10.31], [9.24, 10.0], [20.0, 10.0], [20.88, 10.2], [21.58, 10.77], [21.96, 11.6], [21.94, 12.5], [20.4, 18.5], [20.0, 19.28], [19.31, 19.81], [18.45, 20.0], [4.0, 20.0], [3.0, 19.73], [2.27, 19.0], [2.0, 18.0], [2.0, 5.0], [2.27, 4.0], [3.0, 3.27], [4.0, 3.0], [7.9, 3.0], [8.86, 3.23], [9.59, 3.9], [10.4, 5.1], [11.12, 5.76], [12.07, 6.0], [18.0, 6.0], [19.0, 6.27], [19.73, 7.0], [20.0, 8.0], [20.0, 10.0]], closed: false },
];
const HASH_LINES: [IconLine; 4] = [
    IconLine { points: &[[4.0, 9.0], [20.0, 9.0]], closed: false },
    IconLine { points: &[[4.0, 15.0], [20.0, 15.0]], closed: false },
    IconLine { points: &[[10.0, 3.0], [8.0, 21.0]], closed: false },
    IconLine { points: &[[16.0, 3.0], [14.0, 21.0]], closed: false },
];
const HIGHLIGHTER_LINES: [IconLine; 2] = [
    IconLine { points: &[[9.0, 11.0], [3.0, 17.0], [3.0, 20.0], [12.0, 20.0], [15.0, 17.0]], closed: false },
    IconLine { points: &[[22.0, 12.0], [17.4, 16.6], [16.51, 17.11], [15.49, 17.11], [14.6, 16.6], [9.4, 11.4], [8.89, 10.51], [8.89, 9.49], [9.4, 8.6], [14.0, 4.0]], closed: false },
];
const INFO_LINES: [IconLine; 3] = [
    IconLine { points: &[[22.0, 12.0], [21.69, 14.49], [20.76, 16.82], [19.29, 18.85], [17.36, 20.44], [15.09, 21.51], [12.63, 21.98], [10.13, 21.82], [7.74, 21.05], [5.63, 19.71], [3.91, 17.88], [2.7, 15.68], [2.08, 13.25], [2.08, 10.75], [2.7, 8.32], [3.91, 6.12], [5.63, 4.29], [7.74, 2.95], [10.13, 2.18], [12.63, 2.02], [15.09, 2.49], [17.36, 3.56], [19.29, 5.15], [20.76, 7.18], [21.69, 9.51]], closed: true },
    IconLine { points: &[[12.0, 16.0], [12.0, 12.0]], closed: false },
    IconLine { points: &[[12.0, 8.0], [12.01, 8.0]], closed: false },
];
const KEYBOARD_LINES: [IconLine; 9] = [
    IconLine { points: &[[10.0, 8.0], [10.01, 8.0]], closed: false },
    IconLine { points: &[[12.0, 12.0], [12.01, 12.0]], closed: false },
    IconLine { points: &[[14.0, 8.0], [14.01, 8.0]], closed: false },
    IconLine { points: &[[16.0, 12.0], [16.01, 12.0]], closed: false },
    IconLine { points: &[[18.0, 8.0], [18.01, 8.0]], closed: false },
    IconLine { points: &[[6.0, 8.0], [6.01, 8.0]], closed: false },
    IconLine { points: &[[7.0, 16.0], [17.0, 16.0]], closed: false },
    IconLine { points: &[[8.0, 12.0], [8.01, 12.0]], closed: false },
    IconLine { points: &[[20.0, 4.0], [21.0, 4.27], [21.73, 5.0], [22.0, 6.0], [22.0, 18.0], [21.73, 19.0], [21.0, 19.73], [20.0, 20.0], [4.0, 20.0], [3.0, 19.73], [2.27, 19.0], [2.0, 18.0], [2.0, 6.0], [2.27, 5.0], [3.0, 4.27], [4.0, 4.0]], closed: true },
];
const LISTCHECKS_LINES: [IconLine; 5] = [
    IconLine { points: &[[13.0, 5.0], [21.0, 5.0]], closed: false },
    IconLine { points: &[[13.0, 12.0], [21.0, 12.0]], closed: false },
    IconLine { points: &[[13.0, 19.0], [21.0, 19.0]], closed: false },
    IconLine { points: &[[3.0, 17.0], [5.0, 19.0], [9.0, 15.0]], closed: false },
    IconLine { points: &[[3.0, 7.0], [5.0, 9.0], [9.0, 5.0]], closed: false },
];
const MINUS_LINES: [IconLine; 1] = [
    IconLine { points: &[[5.0, 12.0], [19.0, 12.0]], closed: false },
];
const MONITOR_LINES: [IconLine; 3] = [
    IconLine { points: &[[20.0, 3.0], [21.0, 3.27], [21.73, 4.0], [22.0, 5.0], [22.0, 15.0], [21.73, 16.0], [21.0, 16.73], [20.0, 17.0], [4.0, 17.0], [3.0, 16.73], [2.27, 16.0], [2.0, 15.0], [2.0, 5.0], [2.27, 4.0], [3.0, 3.27], [4.0, 3.0]], closed: true },
    IconLine { points: &[[8.0, 21.0], [16.0, 21.0]], closed: false },
    IconLine { points: &[[12.0, 17.0], [12.0, 21.0]], closed: false },
];
const MOON_LINES: [IconLine; 1] = [
    IconLine { points: &[[20.98, 12.49], [20.57, 14.74], [19.6, 16.82], [18.13, 18.59], [16.26, 19.93], [14.12, 20.75], [11.84, 21.0], [9.57, 20.67], [7.45, 19.77], [5.63, 18.37], [4.23, 16.55], [3.33, 14.43], [3.0, 12.16], [3.25, 9.88], [4.07, 7.74], [5.41, 5.87], [7.17, 4.4], [9.25, 3.43], [11.51, 3.01], [11.84, 3.14], [12.0, 3.45], [11.91, 3.82], [11.2, 5.45], [11.01, 7.23], [11.34, 8.98], [12.18, 10.56], [13.44, 11.82], [15.02, 12.66], [16.77, 12.99], [18.54, 12.8], [20.18, 12.09], [20.55, 12.0], [20.86, 12.15], [20.98, 12.49]], closed: false },
];
const MOVEUPRIGHT_LINES: [IconLine; 2] = [
    IconLine { points: &[[13.0, 5.0], [19.0, 5.0], [19.0, 11.0]], closed: false },
    IconLine { points: &[[19.0, 5.0], [5.0, 19.0]], closed: false },
];
const PACKAGE_LINES: [IconLine; 4] = [
    IconLine { points: &[[11.0, 21.73], [12.0, 22.0], [13.0, 21.73], [20.0, 17.73], [20.73, 17.0], [21.0, 16.0], [21.0, 8.0], [20.73, 7.0], [20.0, 6.27], [13.0, 2.27], [12.0, 2.0], [11.0, 2.27], [4.0, 6.27], [3.27, 7.0], [3.0, 8.0], [3.0, 16.0], [3.27, 17.0], [4.0, 17.73]], closed: true },
    IconLine { points: &[[12.0, 22.0], [12.0, 12.0]], closed: false },
    IconLine { points: &[[3.29, 7.0], [12.0, 12.0], [20.71, 7.0]], closed: false },
    IconLine { points: &[[7.5, 4.27], [16.5, 9.42]], closed: false },
];
const PACKAGEOPEN_LINES: [IconLine; 4] = [
    IconLine { points: &[[12.0, 22.0], [12.0, 13.0]], closed: false },
    IconLine { points: &[[15.17, 2.21], [15.98, 2.0], [16.8, 2.21], [21.0, 4.57], [21.72, 5.28], [21.98, 6.25], [21.72, 7.22], [21.0, 7.93], [8.82, 14.79], [8.0, 15.01], [7.18, 14.79], [3.0, 12.43], [2.28, 11.72], [2.02, 10.75], [2.28, 9.78], [3.0, 9.07]], closed: true },
    IconLine { points: &[[20.0, 13.0], [20.0, 16.87], [19.7, 17.94], [18.89, 18.7], [12.89, 21.78], [12.0, 22.0], [11.11, 21.78], [5.11, 18.7], [4.3, 17.94], [4.0, 16.87], [4.0, 13.0]], closed: false },
    IconLine { points: &[[21.0, 12.43], [21.72, 11.72], [21.98, 10.75], [21.72, 9.78], [21.0, 9.07], [8.83, 2.2], [8.02, 1.98], [7.2, 2.2], [3.0, 4.57], [2.28, 5.28], [2.02, 6.25], [2.28, 7.22], [3.0, 7.93], [15.18, 14.79], [15.99, 15.01], [16.81, 14.79]], closed: true },
];
const PAUSE_LINES: [IconLine; 2] = [
    IconLine { points: &[[18.0, 3.0], [18.71, 3.29], [19.0, 4.0], [19.0, 20.0], [18.71, 20.71], [18.0, 21.0], [15.0, 21.0], [14.29, 20.71], [14.0, 20.0], [14.0, 4.0], [14.29, 3.29], [15.0, 3.0]], closed: true },
    IconLine { points: &[[9.0, 3.0], [9.71, 3.29], [10.0, 4.0], [10.0, 20.0], [9.71, 20.71], [9.0, 21.0], [6.0, 21.0], [5.29, 20.71], [5.0, 20.0], [5.0, 4.0], [5.29, 3.29], [6.0, 3.0]], closed: true },
];
const PENTOOL_LINES: [IconLine; 4] = [
    IconLine { points: &[[18.0, 13.0], [17.35, 8.26], [16.87, 6.85], [15.91, 5.71], [14.59, 5.0], [5.43, 2.1], [4.35, 2.06], [3.41, 2.59], [2.59, 3.41], [2.06, 4.35], [2.1, 5.43], [5.0, 14.59], [5.71, 15.91], [6.85, 16.87], [8.26, 17.35], [13.0, 18.0]], closed: false },
    IconLine { points: &[[18.65, 12.35], [19.19, 12.04], [19.81, 12.04], [20.35, 12.35], [21.65, 13.65], [21.96, 14.19], [21.96, 14.81], [21.65, 15.35], [15.35, 21.65], [14.81, 21.96], [14.19, 21.96], [13.65, 21.65], [12.35, 20.35], [12.04, 19.81], [12.04, 19.19], [12.35, 18.65]], closed: true },
    IconLine { points: &[[3.0, 3.0], [9.59, 9.59]], closed: false },
    IconLine { points: &[[13.0, 11.0], [12.73, 12.0], [12.0, 12.73], [11.0, 13.0], [10.0, 12.73], [9.27, 12.0], [9.0, 11.0], [9.27, 10.0], [10.0, 9.27], [11.0, 9.0], [12.0, 9.27], [12.73, 10.0]], closed: true },
];
const PENCIL_LINES: [IconLine; 2] = [
    IconLine { points: &[[21.17, 6.81], [21.84, 5.75], [21.98, 4.5], [21.57, 3.32], [20.68, 2.43], [19.5, 2.02], [18.25, 2.16], [17.19, 2.83], [3.84, 16.17], [3.54, 16.56], [3.34, 17.0], [2.02, 21.36], [2.15, 21.85], [2.64, 21.98], [7.0, 20.66], [7.44, 20.46], [7.83, 20.16]], closed: true },
    IconLine { points: &[[15.0, 5.0], [19.0, 9.0]], closed: false },
];
const PLAY_LINES: [IconLine; 1] = [
    IconLine { points: &[[5.0, 5.0], [5.27, 4.0], [6.0, 3.27], [7.01, 3.0], [8.01, 3.27], [20.0, 10.27], [20.73, 11.0], [21.0, 12.0], [20.74, 13.0], [20.01, 13.73], [8.01, 20.73], [7.01, 21.0], [6.0, 20.73], [5.27, 20.0], [5.0, 19.0]], closed: true },
];
const PLUS_LINES: [IconLine; 2] = [
    IconLine { points: &[[5.0, 12.0], [19.0, 12.0]], closed: false },
    IconLine { points: &[[12.0, 5.0], [12.0, 19.0]], closed: false },
];
const REDO2_LINES: [IconLine; 2] = [
    IconLine { points: &[[15.0, 14.0], [20.0, 9.0], [15.0, 4.0]], closed: false },
    IconLine { points: &[[20.0, 9.0], [9.5, 9.0], [7.8, 9.27], [6.27, 10.05], [5.05, 11.27], [4.27, 12.8], [4.0, 14.5], [4.27, 16.2], [5.05, 17.73], [6.27, 18.95], [7.8, 19.73], [9.5, 20.0], [13.0, 20.0]], closed: false },
];
const SCAN_LINES: [IconLine; 4] = [
    IconLine { points: &[[3.0, 7.0], [3.0, 5.0], [3.27, 4.0], [4.0, 3.27], [5.0, 3.0], [7.0, 3.0]], closed: false },
    IconLine { points: &[[17.0, 3.0], [19.0, 3.0], [20.0, 3.27], [20.73, 4.0], [21.0, 5.0], [21.0, 7.0]], closed: false },
    IconLine { points: &[[21.0, 17.0], [21.0, 19.0], [20.73, 20.0], [20.0, 20.73], [19.0, 21.0], [17.0, 21.0]], closed: false },
    IconLine { points: &[[7.0, 21.0], [5.0, 21.0], [4.0, 20.73], [3.27, 20.0], [3.0, 19.0], [3.0, 17.0]], closed: false },
];
const SEARCH_LINES: [IconLine; 2] = [
    IconLine { points: &[[21.0, 21.0], [16.66, 16.66]], closed: false },
    IconLine { points: &[[19.0, 11.0], [18.7, 13.16], [17.84, 15.16], [16.46, 16.85], [14.68, 18.1], [12.63, 18.83], [10.45, 18.98], [8.32, 18.54], [6.39, 17.54], [4.79, 16.05], [3.66, 14.19], [3.07, 12.09], [3.07, 9.91], [3.66, 7.81], [4.79, 5.95], [6.39, 4.46], [8.32, 3.46], [10.45, 3.02], [12.63, 3.17], [14.68, 3.9], [16.46, 5.15], [17.84, 6.84], [18.7, 8.84]], closed: true },
];
const SETTINGS_LINES: [IconLine; 2] = [
    IconLine { points: &[[9.67, 4.14], [10.05, 3.06], [10.9, 2.29], [12.0, 2.02], [13.11, 2.29], [13.95, 3.06], [14.33, 4.14], [14.69, 5.19], [15.49, 5.96], [16.56, 6.27], [17.65, 6.05], [18.77, 5.85], [19.85, 6.19], [20.64, 7.01], [20.96, 8.1], [20.72, 9.22], [19.98, 10.08], [19.25, 10.92], [18.98, 12.0], [19.25, 13.08], [19.98, 13.91], [20.72, 14.78], [20.96, 15.89], [20.64, 16.99], [19.85, 17.8], [18.77, 18.15], [17.65, 17.95], [16.56, 17.73], [15.49, 18.04], [14.69, 18.81], [14.33, 19.86], [13.95, 20.94], [13.11, 21.7], [12.0, 21.98], [10.9, 21.7], [10.05, 20.94], [9.67, 19.86], [9.31, 18.81], [8.51, 18.04], [7.44, 17.73], [6.35, 17.95], [5.23, 18.15], [4.15, 17.8], [3.36, 16.99], [3.04, 15.89], [3.28, 14.78], [4.02, 13.91], [4.75, 13.08], [5.02, 12.0], [4.75, 10.92], [4.02, 10.08], [3.29, 9.22], [3.05, 8.11], [3.36, 7.01], [4.15, 6.2], [5.23, 5.85], [6.35, 6.05], [7.44, 6.27], [8.51, 5.96], [9.31, 5.19], [9.67, 4.14]], closed: false },
    IconLine { points: &[[15.0, 12.0], [14.7, 13.3], [13.87, 14.35], [12.67, 14.92], [11.33, 14.92], [10.13, 14.35], [9.3, 13.3], [9.0, 12.0], [9.3, 10.7], [10.13, 9.65], [11.33, 9.08], [12.67, 9.08], [13.87, 9.65], [14.7, 10.7]], closed: true },
];
const SHAPES_LINES: [IconLine; 3] = [
    IconLine { points: &[[8.3, 10.0], [7.82, 9.85], [7.57, 9.41], [7.67, 8.92], [11.4, 3.0], [11.75, 2.69], [12.22, 2.67], [12.6, 2.96], [16.3, 8.9], [16.42, 9.38], [16.19, 9.82], [15.73, 10.0]], closed: true },
    IconLine { points: &[[9.0, 14.0], [9.71, 14.29], [10.0, 15.0], [10.0, 20.0], [9.71, 20.71], [9.0, 21.0], [4.0, 21.0], [3.29, 20.71], [3.0, 20.0], [3.0, 15.0], [3.29, 14.29], [4.0, 14.0]], closed: true },
    IconLine { points: &[[21.0, 17.5], [20.7, 18.92], [19.84, 20.1], [18.58, 20.83], [17.13, 20.98], [15.75, 20.53], [14.67, 19.56], [14.08, 18.23], [14.08, 16.77], [14.67, 15.44], [15.75, 14.47], [17.13, 14.02], [18.58, 14.17], [19.84, 14.9], [20.7, 16.08]], closed: true },
];
const SQUARE_LINES: [IconLine; 1] = [
    IconLine { points: &[[19.0, 3.0], [20.0, 3.27], [20.73, 4.0], [21.0, 5.0], [21.0, 19.0], [20.73, 20.0], [20.0, 20.73], [19.0, 21.0], [5.0, 21.0], [4.0, 20.73], [3.27, 20.0], [3.0, 19.0], [3.0, 5.0], [3.27, 4.0], [4.0, 3.27], [5.0, 3.0]], closed: true },
];
const SQUAREDASHED_LINES: [IconLine; 12] = [
    IconLine { points: &[[5.0, 3.0], [4.0, 3.27], [3.27, 4.0], [3.0, 5.0]], closed: false },
    IconLine { points: &[[19.0, 3.0], [20.0, 3.27], [20.73, 4.0], [21.0, 5.0]], closed: false },
    IconLine { points: &[[21.0, 19.0], [20.73, 20.0], [20.0, 20.73], [19.0, 21.0]], closed: false },
    IconLine { points: &[[5.0, 21.0], [4.0, 20.73], [3.27, 20.0], [3.0, 19.0]], closed: false },
    IconLine { points: &[[9.0, 3.0], [10.0, 3.0]], closed: false },
    IconLine { points: &[[9.0, 21.0], [10.0, 21.0]], closed: false },
    IconLine { points: &[[14.0, 3.0], [15.0, 3.0]], closed: false },
    IconLine { points: &[[14.0, 21.0], [15.0, 21.0]], closed: false },
    IconLine { points: &[[3.0, 9.0], [3.0, 10.0]], closed: false },
    IconLine { points: &[[21.0, 9.0], [21.0, 10.0]], closed: false },
    IconLine { points: &[[3.0, 14.0], [3.0, 15.0]], closed: false },
    IconLine { points: &[[21.0, 14.0], [21.0, 15.0]], closed: false },
];
const SUN_LINES: [IconLine; 9] = [
    IconLine { points: &[[16.0, 12.0], [15.7, 13.53], [14.83, 14.83], [13.53, 15.7], [12.0, 16.0], [10.47, 15.7], [9.17, 14.83], [8.3, 13.53], [8.0, 12.0], [8.3, 10.47], [9.17, 9.17], [10.47, 8.3], [12.0, 8.0], [13.53, 8.3], [14.83, 9.17], [15.7, 10.47]], closed: true },
    IconLine { points: &[[12.0, 2.0], [12.0, 4.0]], closed: false },
    IconLine { points: &[[12.0, 20.0], [12.0, 22.0]], closed: false },
    IconLine { points: &[[4.93, 4.93], [6.34, 6.34]], closed: false },
    IconLine { points: &[[17.66, 17.66], [19.07, 19.07]], closed: false },
    IconLine { points: &[[2.0, 12.0], [4.0, 12.0]], closed: false },
    IconLine { points: &[[20.0, 12.0], [22.0, 12.0]], closed: false },
    IconLine { points: &[[6.34, 17.66], [4.93, 19.07]], closed: false },
    IconLine { points: &[[19.07, 4.93], [17.66, 6.34]], closed: false },
];
const TRASH_LINES: [IconLine; 5] = [
    IconLine { points: &[[10.0, 11.0], [10.0, 17.0]], closed: false },
    IconLine { points: &[[14.0, 11.0], [14.0, 17.0]], closed: false },
    IconLine { points: &[[19.0, 6.0], [19.0, 20.0], [18.73, 21.0], [18.0, 21.73], [17.0, 22.0], [7.0, 22.0], [6.0, 21.73], [5.27, 21.0], [5.0, 20.0], [5.0, 6.0]], closed: false },
    IconLine { points: &[[3.0, 6.0], [21.0, 6.0]], closed: false },
    IconLine { points: &[[8.0, 6.0], [8.0, 4.0], [8.27, 3.0], [9.0, 2.27], [10.0, 2.0], [14.0, 2.0], [15.0, 2.27], [15.73, 3.0], [16.0, 4.0], [16.0, 6.0]], closed: false },
];
const TYPE_LINES: [IconLine; 3] = [
    IconLine { points: &[[12.0, 4.0], [12.0, 20.0]], closed: false },
    IconLine { points: &[[4.0, 7.0], [4.0, 5.0], [4.29, 4.29], [5.0, 4.0], [19.0, 4.0], [19.71, 4.29], [20.0, 5.0], [20.0, 7.0]], closed: false },
    IconLine { points: &[[9.0, 20.0], [15.0, 20.0]], closed: false },
];
const UNDO2_LINES: [IconLine; 2] = [
    IconLine { points: &[[9.0, 14.0], [4.0, 9.0], [9.0, 4.0]], closed: false },
    IconLine { points: &[[4.0, 9.0], [14.5, 9.0], [16.2, 9.27], [17.73, 10.05], [18.95, 11.27], [19.73, 12.8], [20.0, 14.5], [19.73, 16.2], [18.95, 17.73], [17.73, 18.95], [16.2, 19.73], [14.5, 20.0], [11.0, 20.0]], closed: false },
];
const X_LINES: [IconLine; 2] = [
    IconLine { points: &[[18.0, 6.0], [6.0, 18.0]], closed: false },
    IconLine { points: &[[6.0, 6.0], [18.0, 18.0]], closed: false },
];
const ZOOMIN_LINES: [IconLine; 4] = [
    IconLine { points: &[[19.0, 11.0], [18.7, 13.16], [17.84, 15.16], [16.46, 16.85], [14.68, 18.1], [12.63, 18.83], [10.45, 18.98], [8.32, 18.54], [6.39, 17.54], [4.79, 16.05], [3.66, 14.19], [3.07, 12.09], [3.07, 9.91], [3.66, 7.81], [4.79, 5.95], [6.39, 4.46], [8.32, 3.46], [10.45, 3.02], [12.63, 3.17], [14.68, 3.9], [16.46, 5.15], [17.84, 6.84], [18.7, 8.84]], closed: true },
    IconLine { points: &[[21.0, 21.0], [16.65, 16.65]], closed: false },
    IconLine { points: &[[11.0, 8.0], [11.0, 14.0]], closed: false },
    IconLine { points: &[[8.0, 11.0], [14.0, 11.0]], closed: false },
];
const ZOOMOUT_LINES: [IconLine; 3] = [
    IconLine { points: &[[19.0, 11.0], [18.7, 13.16], [17.84, 15.16], [16.46, 16.85], [14.68, 18.1], [12.63, 18.83], [10.45, 18.98], [8.32, 18.54], [6.39, 17.54], [4.79, 16.05], [3.66, 14.19], [3.07, 12.09], [3.07, 9.91], [3.66, 7.81], [4.79, 5.95], [6.39, 4.46], [8.32, 3.46], [10.45, 3.02], [12.63, 3.17], [14.68, 3.9], [16.46, 5.15], [17.84, 6.84], [18.7, 8.84]], closed: true },
    IconLine { points: &[[21.0, 21.0], [16.65, 16.65]], closed: false },
    IconLine { points: &[[8.0, 11.0], [14.0, 11.0]], closed: false },
];
