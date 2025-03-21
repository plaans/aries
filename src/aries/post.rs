use aries::model::Model;

pub trait Post<Lbl> {
    fn post(&self, model: &mut Model<Lbl>);
}
