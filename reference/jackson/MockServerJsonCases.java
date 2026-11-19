import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
// Reads cases (one JSON-escaped input per line) and prints the tree MockServer's ObjectMapperFactory reads (its
// JsonParser features), or the exception: the crate's `ReadFeatures::mockserver()`.
public class MockServerJsonCases {
  public static void main(String[] a) throws Exception {
    ObjectMapper m = new ObjectMapper();
    m.configure(DeserializationFeature.FAIL_ON_TRAILING_TOKENS, false);
    for (JsonParser.Feature f : new JsonParser.Feature[] {
        JsonParser.Feature.ALLOW_COMMENTS, JsonParser.Feature.ALLOW_YAML_COMMENTS,
        JsonParser.Feature.ALLOW_UNQUOTED_FIELD_NAMES, JsonParser.Feature.ALLOW_SINGLE_QUOTES,
        JsonParser.Feature.ALLOW_UNQUOTED_CONTROL_CHARS, JsonParser.Feature.ALLOW_BACKSLASH_ESCAPING_ANY_CHARACTER,
        JsonParser.Feature.ALLOW_NUMERIC_LEADING_ZEROS, JsonParser.Feature.ALLOW_NON_NUMERIC_NUMBERS,
        JsonParser.Feature.ALLOW_MISSING_VALUES, JsonParser.Feature.ALLOW_TRAILING_COMMA }) {
      m.configure(f, true);
    }
    ObjectMapper plain = new ObjectMapper();
    for (String line : Files.readAllLines(Paths.get(a[0]))) {
      String s = plain.readValue(line, String.class);
      String r;
      try { JsonNode n = m.readTree(s); r = "OK " + n.getClass().getSimpleName() + " " + n.toString(); }
      catch (Exception e) { r = "ERR " + e.getClass().getName() + ": " + e.getMessage(); }
      System.out.println(plain.writeValueAsString(r));
    }
  }
}
