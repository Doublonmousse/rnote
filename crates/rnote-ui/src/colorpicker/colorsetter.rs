// Imports
use glib::clone;
use glib::subclass::Signal;
use gtk4::graphene::Rect;
use gtk4::Orientation;
use gtk4::SizeRequestMode;
use gtk4::{
    gdk, gdk::RGBA, glib, prelude::*, subclass::prelude::*, Align, Button, PositionType,
    ToggleButton, Widget,
};
use once_cell::sync::Lazy;
use rnote_compose::Color;
use rnote_engine::ext::GdkRGBAExt;
use std::cell::Cell;
use std::sync::OnceLock;

mod imp {
    use super::*;

    #[derive(Debug)]
    pub(crate) struct RnColorSetter {
        pub(crate) color: Cell<gdk::RGBA>,
        pub(crate) position: Cell<PositionType>,
        pub(crate) active: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RnColorSetter {
        const NAME: &'static str = "RnColorSetter";
        type Type = super::RnColorSetter;
        type ParentType = Widget;

        fn class_init(klass: &mut Self::Class) {
            // Make it look like a GTK button.
            klass.set_css_name("button");
        }
    }

    impl Default for RnColorSetter {
        fn default() -> Self {
            Self {
                color: Cell::new(gdk::RGBA::from_compose_color(
                    super::RnColorSetter::COLOR_DEFAULT,
                )),
                position: Cell::new(PositionType::Right),
                active: Cell::new(false),
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for RnColorSetter {
        // to check whether this has to be used or not
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("right-click")
                        .param_types([i32::static_type()])
                        .build(),
                    Signal::builder("left-click")
                        .param_types([i32::static_type()])
                        .build(),
                    Signal::builder("long-click")
                        .param_types([i32::static_type()])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            let obj = self.obj();
            self.parent_constructed();

            obj.set_hexpand(false);
            obj.set_vexpand(false);
            obj.set_halign(Align::Fill);
            obj.set_valign(Align::Fill);
            obj.add_css_class("flat");
            obj.set_width_request(34);
            obj.set_height_request(34);
            // name collision ? need to use active_button on all properties ??
            //obj.set_active(false);

            // connect a gesture for all interactions
            // Connect a gesture to handle clicks.
            let gesture = gtk4::GestureClick::new();
            gesture.connect_pressed(clone!(@weak obj=> move |_gesture, _, _, _| {
                let val: i32 = 0;
                println!("left click inside closure");

                //obj.set_active(!obj);

                obj.emit_by_name::<()>("left-click", &[&val])
            }));

            let long_click = gtk4::GestureLongPress::new();
            long_click.connect_pressed(clone!(@weak obj => move |ev, x, y| {
                println!("inside closure : pressed {:?} {:?} {:?}", ev, x, y);

                let val: i32 = 0;
                obj.emit_by_name::<()>("long-click", &[&val]);
            }));
            let rightclick_gesture = gtk4::GestureClick::builder()
                .name("rightclick_gesture")
                .button(gdk::BUTTON_SECONDARY)
                .build();
            rightclick_gesture.connect_pressed(clone!(@weak obj => move |_, _, _, _| {
                println!("inside closure : right click");

                let val: i32 = 0;
                obj.emit_by_name::<()>("right-click", &[&val]);
            }));
            obj.add_controller(rightclick_gesture);
            obj.add_controller(long_click);
            obj.add_controller(gesture);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoxed::builder::<gdk::RGBA>("color").build(),
                    glib::ParamSpecEnum::builder_with_default::<PositionType>(
                        "position",
                        PositionType::Right,
                    )
                    .build(),
                    glib::ParamSpecBoolean::builder("active").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "color" => {
                    let color = value
                        .get::<gdk::RGBA>()
                        .expect("value not of type `gdk::RGBA`");
                    self.color.set(color);
                }
                "position" => {
                    let position = value
                        .get::<PositionType>()
                        .expect("value not of type `PositionType`");

                    self.position.replace(position);
                }
                "active" => {
                    let active = value.get::<bool>().expect("value not of type bool");

                    self.active.replace(active);
                }
                _ => panic!("invalid property name"),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "color" => self.color.get().to_value(),
                "position" => self.position.get().to_value(),
                "active" => self.active.get().to_value(),
                _ => panic!("invalid property name"),
            }
        }
    }

    impl WidgetImpl for RnColorSetter {
        fn request_mode(&self) -> SizeRequestMode {
            SizeRequestMode::ConstantSize
        }

        fn measure(&self, orientation: Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                Orientation::Horizontal => (0, 32, -1, -1),
                Orientation::Vertical => (0, 32, -1, -1),
                _ => unimplemented!(),
            }
        }

        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let obj = self.obj();
            let size = (obj.width() as f32, obj.height() as f32);

            // in the background, add the transparency checkboard

            //then the color
            // parse the color
            let color: gdk::RGBA = self.color.get();

            snapshot.append_color(&color, &Rect::new(0.0, 0.0, size.0, size.1));
            // and a bar on the bottom that signifies the button is activated
            let colorsetter_fg_color = if color.alpha() == 0.0 {
                RGBA::new(0.0, 0.0, 0.0, 1.0)
            }
            //else if  < color::FG_LUMINANCE_THRESHOLD {
            //RGBA::new(1.0, 1.0, 1.0, 1.0)
            //}
            // todo : find the corresponding methods and convert if needed
            else {
                RGBA::new(0.0, 0.0, 0.0, 1.0)
            };

            if self.active.get() {
                snapshot.append_color(
                    &colorsetter_fg_color,
                    &Rect::new(0.0, 0.9 * size.1, size.0, 0.2 * size.1),
                );
            }
        }
    }

    impl ButtonImpl for RnColorSetter {}

    impl ToggleButtonImpl for RnColorSetter {}
}

glib::wrapper! {
    pub(crate) struct RnColorSetter(ObjectSubclass<imp::RnColorSetter>)
        @extends ToggleButton, Button, Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for RnColorSetter {
    fn default() -> Self {
        Self::new()
    }
}

impl RnColorSetter {
    pub(crate) const COLOR_DEFAULT: Color = Color::BLACK;

    pub(crate) fn new() -> Self {
        glib::Object::new()
    }

    #[allow(unused)]
    pub(crate) fn position(&self) -> PositionType {
        self.property::<PositionType>("position")
    }

    #[allow(unused)]
    pub(crate) fn set_position(&self, position: PositionType) {
        self.set_property("position", position.to_value());
    }

    #[allow(unused)]
    pub(crate) fn color(&self) -> gdk::RGBA {
        self.property::<gdk::RGBA>("color")
    }

    #[allow(unused)]
    pub(crate) fn set_color(&self, color: gdk::RGBA) {
        self.set_property("color", color.to_value());
    }
}
