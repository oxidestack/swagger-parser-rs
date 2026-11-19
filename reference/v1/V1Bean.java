import com.fasterxml.jackson.databind.JsonNode;
import io.swagger.models.*;
import io.swagger.models.parameters.Parameter;
import io.swagger.models.properties.Property;
import io.swagger.util.Json;
import java.nio.file.Files;
import java.nio.file.Paths;
// swagger-core 1.x Json.mapper().convertValue(node, type) followed by Json.mapper().writeValueAsString(result),
// for each {"type": ..., "value": ...} of the JSON array in the given file. Mirrors examples/v1_bean.rs.
public class V1Bean {
  public static void main(String[] a) throws Exception {
    JsonNode cases = Json.mapper().readTree(new String(Files.readAllBytes(Paths.get(a[0])), "UTF-8"));
    for (JsonNode c : cases) {
      String t = c.get("type").asText();
      Class<?> cls = t.equals("Model") ? Model.class : t.equals("Parameter") ? Parameter.class : t.equals("Response") ? Response.class : t.equals("Path") ? io.swagger.models.Path.class : Property.class;
      String r;
      try {
        Object o = Json.mapper().convertValue(c.get("value"), cls);
        r = o == null ? "null" : Json.mapper().writeValueAsString(o);
      } catch (Throwable e) { r = "THROW " + e.getClass().getName(); }
      System.out.println(t + " " + r);
    }
  }
}
