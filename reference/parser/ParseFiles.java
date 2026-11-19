import io.swagger.v3.core.util.*;
import io.swagger.v3.parser.*;
import io.swagger.v3.parser.core.models.*;
import java.nio.file.*;
import java.security.*;
// For each file: readContents(content, null, options[resolve=false]) → Json.pretty/Json31.pretty + messages.
// Args: fileList [text]. Mirrors examples/parse_files.rs.
public class ParseFiles {
  public static void main(String[] a) throws Exception {
    boolean text = a.length > 1 && a[1].equals("text");
    boolean rt = a.length > 2 || (a.length > 1 && a[1].equals("rt"));
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      String s = new String(Files.readAllBytes(Paths.get(f)), "UTF-8");
      ParseOptions o = new ParseOptions();
      String r;
      try {
        SwaggerParseResult res = new OpenAPIV3Parser().readContents(s, null, o);
        String model = res.getOpenAPI() == null ? "null" : (res.isOpenapi31() ? Json31.pretty(res.getOpenAPI()) : Json.pretty(res.getOpenAPI()));
        if (rt && res.getOpenAPI() != null) {
          com.fasterxml.jackson.databind.ObjectMapper m = res.isOpenapi31() ? Json31.mapper() : Json.mapper();
          io.swagger.v3.oas.models.OpenAPI back = m.readValue(model, io.swagger.v3.oas.models.OpenAPI.class);
          model = res.isOpenapi31() ? Json31.pretty(back) : Json.pretty(back);
        }
        r = model + "\nMESSAGES " + res.getMessages();
      } catch (Throwable e) { r = "THROW " + e; }
      if (text) { System.out.println("=== " + f + "\n" + r); continue; }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(r.getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
