import com.fasterxml.jackson.databind.*;
import java.nio.file.*;
import java.util.*;
// Reads cases (one hex-encoded byte input per line) and prints, JSON-escaped, what `new ObjectMapper().readTree(byte[])`
// makes of each: the node class and toString(), or the exception class and message. Run in the webhook image's JVM
// (9.1.0: Jackson 2.22.3, a jlinked JRE without jdk.compiler, so compile first with `javac --release 17` against
// jackson-databind/core/annotations 2.22.3 into target/jbc, on the arm64 variant):
//   docker run --rm --platform linux/arm64 --entrypoint java -v "$PWD":/w mockserver/mockserver-webhook:9.1.0 \
//     -cp /mockserver-webhook.jar:/w/target/jbc JsonBytesCases /w/crates/jackson/tests/fixtures/json-bytes-cases.txt
public class JsonBytesCases {
  public static void main(String[] a) throws Exception {
    ObjectMapper mapper = new ObjectMapper();
    ObjectWriter out = mapper.writer().with(com.fasterxml.jackson.core.json.JsonWriteFeature.ESCAPE_NON_ASCII);
    for (String line : Files.readAllLines(Paths.get(a[0]))) {
      byte[] in = HexFormat.of().parseHex(line.trim());
      String r;
      try { JsonNode n = mapper.readTree(in); r = "OK " + n.getClass().getSimpleName() + " " + n.toString(); }
      catch (Exception e) { r = "ERR " + e.getClass().getName() + ": " + e.getMessage(); }
      System.out.println(out.writeValueAsString(r));
    }
  }
}
