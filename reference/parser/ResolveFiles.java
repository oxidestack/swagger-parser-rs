import io.swagger.v3.core.util.*;
import io.swagger.v3.parser.*;
import io.swagger.v3.parser.core.models.*;
import java.nio.file.*;
import java.security.*;
// For each file: readLocation(file, null, options) → Json.pretty/Json31.pretty + messages.
// Args: fileList mode [text]; mode = resolve (resolve=true) | full (MockServer's options: resolve, resolveFully,
// resolveCombinators, skipMatches, allowEmptyString, camelCaseFlattenNaming). Mirrors examples/resolve_files.rs.
public class ResolveFiles {
  public static void main(String[] a) throws Exception {
    String mode = a[1];
    boolean text = a.length > 2 && a[2].equals("text");
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      ParseOptions o = new ParseOptions();
      o.setResolve(true);
      if (mode.equals("full")) {
        o.setResolveFully(true);
        o.setResolveCombinators(true);
        o.setSkipMatches(true);
        o.setAllowEmptyString(true);
        o.setCamelCaseFlattenNaming(true);
      }
      if (mode.startsWith("flatten")) o.setFlatten(true);
      if (mode.equals("flattenc")) { o.setFlattenComposedSchemas(true); o.setCamelCaseFlattenNaming(true); }
      if (mode.equals("flattens")) { o.setSkipMatches(true); o.setResolveFully(true); }
      String r;
      try {
        SwaggerParseResult res = new OpenAPIV3Parser().readLocation(f, null, o);
        String model = res.getOpenAPI() == null ? "null" : (res.isOpenapi31() ? Json31.pretty(res.getOpenAPI()) : Json.pretty(res.getOpenAPI()));
        r = model + "\nMESSAGES " + res.getMessages();
      } catch (Throwable e) { r = "THROW " + e; }
      if (text) { System.out.println("=== " + f + "\n" + r); continue; }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(r.getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
