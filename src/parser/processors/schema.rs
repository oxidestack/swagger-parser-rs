//! `SchemaProcessor`

use super::mocks;
use super::ExternalRefProcessor;
use crate::java::{JMap, JResult};
use crate::models::*;
use crate::parser::ref_utils::{compute_definition_name, compute_ref_format, is_an_external_ref_format, RefType};
use crate::parser::resolver_cache::Cache;
use std::rc::Rc;

/// Port of `io.swagger.v3.parser.processors.SchemaProcessor`: resolves external `$ref`s in a schema and every schema nested in it.
pub struct SchemaProcessor {
    external_ref_processor: ExternalRefProcessor,
    #[allow(dead_code)]
    openapi31: bool,
}

impl SchemaProcessor {
    /// `new SchemaProcessor(cache, openAPI)`
    pub fn new(cache: Rc<dyn Cache>, open_api: &OpenAPI) -> SchemaProcessor {
        SchemaProcessor::new31(cache, open_api, false)
    }

    /// `new SchemaProcessor(cache, openAPI, openapi31)`
    pub fn new31(cache: Rc<dyn Cache>, open_api: &OpenAPI, openapi31: bool) -> SchemaProcessor {
        let _constructing = mocks::constructed("SchemaProcessor");
        SchemaProcessor { external_ref_processor: ExternalRefProcessor::new(cache, open_api), openapi31 }
    }

    /// `processSchema(schema)` (no `JsonSchemaParserExtension`s are registered)
    pub fn process_schema(&self, schema: Option<&Schema>) -> JResult<()> {
        let Some(schema) = schema else { return Ok(()) };
        if let Some(h) = mocks::current().process_schema && let Some(r) = h(schema) {
            return r;
        }
        if schema.get_ref().is_some() {
            self.process_reference_schema(schema)
        } else {
            self.process_schema_type(Some(schema))
        }
    }

    /// `processSchemaType(schema)`
    pub fn process_schema_type(&self, schema: Option<&Schema>) -> JResult<()> {
        let Some(schema) = schema else { return Ok(()) };
        if schema.is_array_schema() {
            self.process_array_schema(schema)?;
        }
        if schema.is_composed_schema() {
            self.process_composed_schema(schema)?;
        }
        if schema.get_properties().is_some_and(|p| !p.is_empty()) {
            self.process_property_schema(schema)?;
        }
        if schema.get_not().is_some() {
            self.process_not_schema(schema)?;
        }
        if schema.get_additional_properties().is_some() {
            self.process_additional_properties(schema)?;
        }
        if schema.get_discriminator().is_some() {
            self.process_discriminator_schema(schema)?;
        }
        Ok(())
    }

    fn process_discriminator_schema(&self, schema: &Schema) -> JResult<()> {
        if let Some(mapping) = schema.get_discriminator().and_then(|d| d.get_mapping()) {
            for r in mapping.values() {
                self.process_reference_schema(&Schema::new().ref_(r))?;
            }
        }
        Ok(())
    }

    fn process_additional_properties(&self, schema: &Schema) -> JResult<()> {
        if let Some(AdditionalProperties::Schema(ap)) = schema.get_additional_properties() {
            if ap.get_ref().is_some() {
                self.process_reference_schema(&ap)?;
            } else {
                self.process_schema_type(Some(&ap))?;
            }
        }
        Ok(())
    }

    fn process_not_schema(&self, schema: &Schema) -> JResult<()> {
        if let Some(not) = schema.get_not() {
            if not.get_ref().is_some() {
                self.process_reference_schema(&not)?;
            } else {
                self.process_schema_type(Some(&not))?;
            }
        }
        Ok(())
    }

    /// `processPropertySchema(schema)`
    pub fn process_property_schema(&self, schema: &Schema) -> JResult<()> {
        if schema.get_ref().is_some() {
            self.process_reference_schema(schema)?;
        }
        if let Some(props) = schema.get_properties() {
            for (_, property) in props.entries() {
                if property.get_ref().is_some() {
                    self.process_reference_schema(&property)?;
                } else {
                    self.process_schema_type(Some(&property))?;
                }
            }
        }
        Ok(())
    }

    /// `processComposedSchema(composedSchema)`
    pub fn process_composed_schema(&self, composed: &Schema) -> JResult<()> {
        if let Some(all_of) = composed.get_all_of() {
            for s in all_of.to_vec() {
                if s.get_ref().is_some() {
                    self.process_reference_schema(&s)?;
                } else {
                    self.process_schema_type(Some(&s))?;
                }
            }
        }
        if composed.get_one_of().is_some() || composed.get_any_of().is_some() {
            for l in [composed.get_one_of(), composed.get_any_of()].into_iter().flatten() {
                for s in l.to_vec() {
                    if let Some(old_ref) = s.get_ref() {
                        self.process_reference_schema(&s)?;
                        let new_ref = s.get_ref();
                        change_discriminator_mapping(composed, &old_ref, new_ref.as_deref())?;
                    } else {
                        self.process_schema_type(Some(&s))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `processArraySchema(arraySchema)`
    pub fn process_array_schema(&self, array: &Schema) -> JResult<()> {
        let items = array.get_items();
        match &items {
            Some(i) if i.get_ref().is_some() => self.process_reference_schema(i),
            _ => self.process_schema_type(items.as_ref()),
        }
    }

    fn process_reference_schema(&self, schema: &Schema) -> JResult<()> {
        let r = schema.get_ref().unwrap_or_default();
        let format = compute_ref_format(&r);
        if is_an_external_ref_format(format)
            && let Some(new_ref) = self.external_ref_processor.process_ref_to_external_schema(&r, format)?
        {
            schema.set_ref(format!("{}{new_ref}", RefType::Schemas.internal_prefix()));
        }
        Ok(())
    }
}

fn change_discriminator_mapping(composed: &Schema, old_ref: &str, new_ref: Option<&str>) -> JResult<()> {
    let Some(discriminator) = composed.get_discriminator() else { return Ok(()) };
    if Some(old_ref) == new_ref {
        return Ok(());
    }
    let old_name = compute_definition_name(old_ref)?;
    // computeDefinitionName(null) → NPE
    let Some(new_ref) = new_ref else {
        return Err(crate::java::JavaException::npe("Cannot invoke \"String.split(String)\" because \"ref\" is null"));
    };
    let new_name = compute_definition_name(new_ref)?;
    let mut mapping_name: Option<String> = None;
    if let Some(mapping) = discriminator.get_mapping() {
        for name in mapping.keys() {
            if mapping.get(&name).as_deref() == Some(old_ref) {
                mapping_name = Some(name);
                break;
            }
        }
        if let Some(n) = &mapping_name {
            mapping.put(n.clone(), new_ref.to_string());
        }
    }
    if mapping_name.is_none() && old_name != new_name {
        if discriminator.get_mapping().is_none() {
            discriminator.set_mapping(JMap::new_hash());
        }
        discriminator.get_mapping().unwrap().put(old_name, new_ref.to_string());
    }
    Ok(())
}
