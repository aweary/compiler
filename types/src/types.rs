
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type<Struct, Effect, Parameter> {
    Unknown,
    Number,
    String,
    Boolean,
    Unit,
    Struct(Struct),
    Effect(Effect),
    Parameter(Parameter),
}
