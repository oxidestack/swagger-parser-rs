import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
// Reads JSON-string-encoded YAML inputs (one per line) and prints JSON-encoded YamlDump results (custom + default resolver).
public class YamlCases {
  public static void main(String[] a) throws Exception {
    ObjectMapper m = new ObjectMapper();
    for (String line : Files.readAllLines(Paths.get(a[0]))) {
      String s = m.readValue(line, String.class);
      System.out.println(m.writeValueAsString(YamlDump.run(s, true) + "\n" + YamlDump.run(s, false)));
    }
  }
}
