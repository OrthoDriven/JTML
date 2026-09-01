#[derive(Default)]
pub enum RotationRepresentation {
    AxisAngle = 0,
    #[default]
    Euler = 1,
}

#[derive(Default)]
pub enum TranslationRepresentation {
    // This defines how we want to structure the translation component of the pose
    // optimization given the following intuition: When a human is optimizing the pose,
    // there is roughly an x/y component for in-plane translation, and a "size" component.
    // Because of perspective projection, the "size" component only maps onto the "Z" part
    // of the translation when the object is at the principal point. Thus, we're going to
    // make it possible to have the "Z" of the pose (in hyperbox space) represent a
    // translation along the ray from the camera-->object, such that adjusting this
    // parameter matches more closely to size.
    #[default]
    PureEuclidean = 0,
    CameraCentered = 1,
}

#[derive(Default)]
pub enum RefinementOptions {
    #[default]
    NoRefinement = 0,
    BOBYQA = 1,
}

#[derive(Clone, Copy)]
pub struct MinBoxSize {
    pub values: [Option<f64>; 6],
}

impl Default for MinBoxSize {
    fn default() -> Self {
        Self {
            values: [
                Some(0.1),
                Some(0.1),
                Some(0.1),
                Some(0.1),
                Some(0.1),
                Some(0.1),
            ],
        }
    }
}

#[derive(Default)]
pub enum POHSettings {
    #[default]
    ConvexHull = 0,
    Pareto = 1,
}

#[derive(Default)]
pub struct DirectSettings {
    pub poh_selection_strategy: POHSettings,
    pub min_box_size: MinBoxSize,
    pub rotation_style: RotationRepresentation,
    pub translation_style: TranslationRepresentation,
    pub refinement: RefinementOptions,
}
