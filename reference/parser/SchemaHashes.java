import io.swagger.v3.parser.*;
import io.swagger.v3.parser.core.models.*;
import io.swagger.v3.oas.models.media.Schema;
import java.nio.file.*;
import java.util.*;
// For each file: readContents without resolution, then hashCode() of every components schema and path item.
public class SchemaHashes {
  public static void main(String[] a) throws Exception {
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      String s = new String(Files.readAllBytes(Paths.get(f)), "UTF-8");
      try {
        SwaggerParseResult res = new OpenAPIV3Parser().readContents(s, null, new ParseOptions());
        if (res.getOpenAPI() == null || res.getOpenAPI().getComponents() == null || res.getOpenAPI().getComponents().getSchemas() == null) continue;
        for (Map.Entry<String, Schema> e : res.getOpenAPI().getComponents().getSchemas().entrySet()) {
          System.out.println(f + " " + e.getKey() + " " + (e.getValue() == null ? 0 : e.getValue().hashCode()));
        }
      } catch (Throwable e) { System.out.println(f + " THROW"); }
    }
  }
}
