import io.swagger.v3.core.util.*;
import io.swagger.v3.parser.*;
import io.swagger.v3.parser.core.models.*;
import java.nio.file.*;
import java.security.*;
// For each file: readLocation(file, null, resolve=true) → Yaml.pretty/Yaml31.pretty. Args: fileList [text]
public class YamlOut {
  public static void main(String[] a) throws Exception {
    boolean text = a.length > 1 && a[1].equals("text");
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      ParseOptions o = new ParseOptions();
      o.setResolve(true);
      String r;
      try {
        SwaggerParseResult res = new OpenAPIV3Parser().readLocation(f, null, o);
        r = res.getOpenAPI() == null ? "null" : (res.isOpenapi31() ? Yaml31.pretty(res.getOpenAPI()) : Yaml.pretty(res.getOpenAPI()));
      } catch (Throwable e) { r = "THROW " + e; }
      if (text) { System.out.println("=== " + f + "\n" + r); continue; }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(String.valueOf(r).getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
