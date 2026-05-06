use schemars::{Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    value.parse::<u64>().map_err(serde::de::Error::custom)
}

pub fn json_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "string",
        "pattern": "^[0-9]+$",
        "maxLength": 20
    })
}

pub mod option {
    use schemars::{Schema, SchemaGenerator, json_schema};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        value
            .map(|value| value.parse::<u64>().map_err(serde::de::Error::custom))
            .transpose()
    }

    pub fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "anyOf": [
                {
                    "type": "string",
                    "pattern": "^[0-9]+$",
                    "maxLength": 20
                },
                {
                    "type": "null"
                }
            ]
        })
    }
}
