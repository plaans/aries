use aries::model::Model;

/// Used to post aries constraint into aries model.
pub trait Post<Lbl> {
    /// Post the constraint into aries model.
    fn post(&self, model: &mut Model<Lbl>);
}
