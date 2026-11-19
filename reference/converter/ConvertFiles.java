import io.swagger.parser.OpenAPIParser;
import io.swagger.v3.core.util.*;
import io.swagger.v3.parser.converter.SwaggerConverter;
import io.swagger.v3.parser.core.models.*;
import java.nio.file.*;
import java.security.*;
// For each file: SwaggerConverter / OpenAPIParser readLocation or readContents → Json.pretty + messages.
// Args: fileList mode [text]; mode = <entry>-<opts>:
//   entry: conv (new SwaggerConverter().readLocation), contents (new SwaggerConverter().readContents(file text)),
//          parser (new OpenAPIParser().readLocation)
//   opts:  none (null options), plain (new ParseOptions()), resolve, full (MockServer's options), flatten
// Mirrors examples/convert_files.rs.
public class ConvertFiles {
  public static void main(String[] a) throws Exception {
    String[] mode = a[1].split("-");
    boolean text = a.length > 2 && a[2].equals("text");
    for (String f : Files.readAllLines(Paths.get(a[0]))) {
      ParseOptions o = new ParseOptions();
      if (!mode[1].equals("plain")) o.setResolve(true);
      if (mode[1].equals("full")) {
        o.setResolveFully(true);
        o.setResolveCombinators(true);
        o.setSkipMatches(true);
        o.setAllowEmptyString(true);
        o.setCamelCaseFlattenNaming(true);
      }
      if (mode[1].equals("flatten")) o.setFlatten(true);
      if (mode[1].equals("none")) o = null;
      String r;
      try {
        SwaggerParseResult res;
        if (mode[0].equals("conv")) res = new SwaggerConverter().readLocation(f, null, o);
        else if (mode[0].equals("contents")) res = new SwaggerConverter().readContents(new String(Files.readAllBytes(Paths.get(f)), "UTF-8"), null, o);
        else res = new OpenAPIParser().readLocation(f, null, o);
        if (res == null) r = "RESULT null";
        else {
          String model = res.getOpenAPI() == null ? "null" : (res.isOpenapi31() ? Json31.pretty(res.getOpenAPI()) : Json.pretty(res.getOpenAPI()));
          r = model + "\nMESSAGES " + res.getMessages();
        }
      } catch (Throwable e) { r = "THROW " + e; }
      if (text) { System.out.println("=== " + f + "\n" + r); continue; }
      byte[] d = MessageDigest.getInstance("SHA-256").digest(r.getBytes("UTF-8"));
      StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b));
      System.out.println(sb + " " + f);
    }
  }
}
