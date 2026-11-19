use monty_types::MontyObject;

/// A record with named fields: `type_name(field=value, ...)`, so `.field` works in Python.
pub fn record(type_name: &str, fields: Vec<(&str, MontyObject)>) -> MontyObject {
    let names: Vec<String> = fields.iter().map(|(n, _)| n.to_string()).collect();
    MontyObject::named_tuple(type_name, names, fields.into_iter().map(|(_, v)| v))
}

pub fn opt_int(v: Option<i32>) -> MontyObject {
    v.map_or_else(MontyObject::none, |c| MontyObject::int(c.into()))
}
