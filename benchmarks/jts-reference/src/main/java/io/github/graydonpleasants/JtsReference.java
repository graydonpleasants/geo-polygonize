package io.github.graydonpleasants;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.locationtech.jts.geom.Coordinate;
import org.locationtech.jts.geom.GeometryFactory;
import org.locationtech.jts.geom.LineString;
import org.locationtech.jts.geom.Polygon;
import org.locationtech.jts.geom.PrecisionModel;
import org.locationtech.jts.noding.snapround.GeometryNoder;
import org.locationtech.jts.operation.polygonize.Polygonizer;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

public final class JtsReference {
    private static final String JTS_VERSION = "1.20.0";
    private static final String JACKSON_VERSION = "2.19.2";
    private static final String LANE =
            "certified-fixed-precision-noding-plus-polygonization";
    private static final ObjectMapper JSON = new ObjectMapper();

    private JtsReference() {}

    public static void main(String[] arguments) throws Exception {
        Map<String, String> args = arguments(arguments);
        Path root = Path.of(args.getOrDefault("--root", ".")).toAbsolutePath();
        String workloadId = required(args, "--workload");
        Path output = Path.of(required(args, "--output"));
        Path manifestPath = args.containsKey("--manifest")
                ? Path.of(args.get("--manifest")).toAbsolutePath()
                : root.resolve("crates/geo-polygonize-core/tests/workloads/manifest-v1.json");
        Path manifestDirectory = manifestPath.getParent();
        if (manifestDirectory == null) {
            throw new IllegalArgumentException("manifest path has no parent directory");
        }
        JsonNode workload = workload(
                JSON.readTree(manifestPath.toFile()), workloadId);
        double gridSize = certifiedGridSize(workload);
        Path clipPath = manifestDirectory.resolve(workload.at("/artifact/clip_path").asText());
        verifyArtifactSha256(clipPath, workload.at("/artifact/sha256").asText());
        List<LineString> input = inputSegments(
                JSON.readTree(clipPath.toFile()),
                new GeometryFactory());

        PrecisionModel precision = new PrecisionModel(1.0 / gridSize);
        GeometryNoder noder = new GeometryNoder(precision);
        noder.setValidate(true);
        @SuppressWarnings("unchecked")
        List<LineString> noded = noder.node(input);
        List<LineString> unique = uniqueSegments(noded, new GeometryFactory(precision));

        Polygonizer polygonizer = new Polygonizer();
        polygonizer.add(unique);
        Map<String, Object> topology = topology(polygonizer);
        String fingerprint = HexFormat.of().formatHex(
                MessageDigest.getInstance("SHA-256").digest(JSON.writeValueAsBytes(topology)));

        Map<String, Object> implementation = new LinkedHashMap<>();
        implementation.put("name", "jts");
        implementation.put("version", JTS_VERSION);
        implementation.put("dependencies", Map.of("jackson-databind", JACKSON_VERSION));

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("schema_version", 1);
        result.put("workload_id", workloadId);
        result.put("lane", LANE);
        result.put("implementation", implementation);
        result.put("fingerprint_sha256", fingerprint);
        result.put("topology", topology);
        JSON.writerWithDefaultPrettyPrinter().writeValue(output.toFile(), result);
    }

    private static Map<String, String> arguments(String[] values) {
        if (values.length % 2 != 0) {
            throw new IllegalArgumentException("arguments must use --name value pairs");
        }
        Map<String, String> result = new LinkedHashMap<>();
        for (int index = 0; index < values.length; index += 2) {
            if (!values[index].startsWith("--")
                    || result.put(values[index], values[index + 1]) != null) {
                throw new IllegalArgumentException("invalid or duplicate argument " + values[index]);
            }
        }
        return result;
    }

    private static String required(Map<String, String> args, String name) {
        String value = args.get(name);
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException(name + " is required");
        }
        return value;
    }

    private static JsonNode workload(JsonNode manifest, String id) {
        for (JsonNode workload : manifest.path("workloads")) {
            if (id.equals(workload.path("id").asText())) {
                if (!"parity".equals(workload.path("compatibility_class").asText())
                        || !contains(workload.path("permitted_profiles"), "certified-fixed")) {
                    throw new IllegalArgumentException(id + " is not a certified-fixed parity workload");
                }
                return workload;
            }
        }
        throw new IllegalArgumentException("unknown workload " + id);
    }

    private static boolean contains(JsonNode values, String expected) {
        for (JsonNode value : values) {
            if (expected.equals(value.asText())) {
                return true;
            }
        }
        return false;
    }

    private static void verifyArtifactSha256(Path path, String expected) throws Exception {
        if (!expected.matches("[0-9a-f]{64}")) {
            throw new IllegalArgumentException("workload artifact SHA-256 must use lowercase hex");
        }
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (var input = Files.newInputStream(path)) {
            byte[] buffer = new byte[64 * 1024];
            int read;
            while ((read = input.read(buffer)) != -1) {
                digest.update(buffer, 0, read);
            }
        }
        String observed = HexFormat.of().formatHex(digest.digest());
        if (!observed.equals(expected)) {
            throw new IllegalArgumentException(
                    "workload artifact checksum mismatch for " + path
                            + ": expected " + expected + ", observed " + observed);
        }
    }

    private static double certifiedGridSize(JsonNode workload) {
        for (JsonNode options : workload.path("options")) {
            JsonNode precision = options.path("precision_model");
            if ("fixed_grid".equals(precision.path("type").asText())
                    && "CertifiedFixedPrecision".equals(
                            options.at("/noding/guarantee").asText())) {
                double gridSize = precision.path("grid_size").asDouble();
                if (Double.isFinite(gridSize) && gridSize > 0.0) {
                    return gridSize;
                }
            }
        }
        throw new IllegalArgumentException("workload has no certified fixed-precision options");
    }

    private static List<LineString> inputSegments(JsonNode collection, GeometryFactory factory) {
        List<LineString> result = new ArrayList<>();
        for (JsonNode feature : collection.path("features")) {
            JsonNode geometry = feature.path("geometry");
            String type = geometry.path("type").asText();
            JsonNode coordinates = geometry.path("coordinates");
            if ("LineString".equals(type)) {
                addSegments(coordinates, factory, result);
            } else if ("MultiLineString".equals(type)) {
                for (JsonNode line : coordinates) {
                    addSegments(line, factory, result);
                }
            } else {
                throw new IllegalArgumentException("workload geometry must contain line strings");
            }
        }
        return result;
    }

    private static void addSegments(
            JsonNode coordinates, GeometryFactory factory, List<LineString> result) {
        for (int index = 1; index < coordinates.size(); index++) {
            result.add(factory.createLineString(new Coordinate[] {
                    coordinate(coordinates.get(index - 1)), coordinate(coordinates.get(index))
            }));
        }
    }

    private static Coordinate coordinate(JsonNode value) {
        if (value.size() < 2) {
            throw new IllegalArgumentException("GeoJSON position must contain x and y");
        }
        double x = value.get(0).asDouble();
        double y = value.get(1).asDouble();
        if (!Double.isFinite(x) || !Double.isFinite(y)) {
            throw new IllegalArgumentException("GeoJSON coordinates must be finite");
        }
        return new Coordinate(x, y);
    }

    private static List<LineString> uniqueSegments(
            List<LineString> lines, GeometryFactory factory) {
        Map<String, LineString> unique = new TreeMap<>();
        for (LineString line : lines) {
            Coordinate[] coordinates = line.getCoordinates();
            for (int index = 1; index < coordinates.length; index++) {
                Coordinate first = coordinates[index - 1];
                Coordinate second = coordinates[index];
                CoordinateBits left = CoordinateBits.of(first);
                CoordinateBits right = CoordinateBits.of(second);
                if (left.equals(right)) {
                    continue;
                }
                String key = left.compareTo(right) <= 0
                        ? left + "|" + right
                        : right + "|" + left;
                unique.putIfAbsent(
                        key,
                        factory.createLineString(new Coordinate[] {
                                new Coordinate(first.x, first.y),
                                new Coordinate(second.x, second.y)
                        }));
            }
        }
        return new ArrayList<>(unique.values());
    }

    private static Map<String, Object> topology(Polygonizer polygonizer) throws Exception {
        List<Object> polygons = new ArrayList<>();
        for (Object value : polygonizer.getPolygons()) {
            Polygon polygon = (Polygon) value;
            List<Object> interiors = new ArrayList<>();
            for (int index = 0; index < polygon.getNumInteriorRing(); index++) {
                interiors.add(canonicalRing(polygon.getInteriorRingN(index).getCoordinates()));
            }
            interiors.sort(Comparator.comparing(JtsReference::compactUnchecked));
            Map<String, Object> result = new LinkedHashMap<>();
            result.put("exterior", canonicalRing(polygon.getExteriorRing().getCoordinates()));
            result.put("interiors", interiors);
            polygons.add(result);
        }
        polygons.sort(Comparator.comparing(JtsReference::compactUnchecked));

        Map<String, Object> result = new LinkedHashMap<>();
        result.put("polygons", polygons);
        result.put("dangles", canonicalLines(polygonizer.getDangles(), false));
        result.put("cut_edges", canonicalLines(polygonizer.getCutEdges(), false));
        result.put("invalid_rings", canonicalLines(polygonizer.getInvalidRingLines(), true));
        return result;
    }

    private static List<Object> canonicalLines(Collection<?> values, boolean rings)
            throws Exception {
        Map<String, Object> result = new TreeMap<>();
        for (Object value : values) {
            Coordinate[] coordinates = ((LineString) value).getCoordinates();
            Object line = rings ? canonicalRing(coordinates) : canonicalLine(coordinates);
            result.putIfAbsent(JSON.writeValueAsString(line), line);
        }
        return new ArrayList<>(result.values());
    }

    private static List<Map<String, String>> canonicalLine(Coordinate[] coordinates) {
        List<CoordinateBits> forward = coordinateBits(coordinates);
        List<CoordinateBits> reverse = new ArrayList<>(forward.reversed());
        return maps(compare(forward, reverse) <= 0 ? forward : reverse);
    }

    private static List<Map<String, String>> canonicalRing(Coordinate[] coordinates) {
        List<CoordinateBits> ring = coordinateBits(coordinates);
        if (ring.size() > 1 && ring.getFirst().equals(ring.getLast())) {
            ring.removeLast();
        }
        if (ring.isEmpty()) {
            return List.of();
        }
        List<CoordinateBits> forward = rotateMinimum(ring);
        List<CoordinateBits> reverse = rotateMinimum(new ArrayList<>(ring.reversed()));
        List<CoordinateBits> result = new ArrayList<>(
                compare(forward, reverse) <= 0 ? forward : reverse);
        result.add(result.getFirst());
        return maps(result);
    }

    private static List<CoordinateBits> coordinateBits(Coordinate[] coordinates) {
        List<CoordinateBits> result = new ArrayList<>(coordinates.length);
        for (Coordinate coordinate : coordinates) {
            result.add(CoordinateBits.of(coordinate));
        }
        return result;
    }

    private static List<CoordinateBits> rotateMinimum(List<CoordinateBits> values) {
        int size = values.size();
        if (size < 2) {
            return new ArrayList<>(values);
        }
        int left = 0;
        int right = 1;
        int offset = 0;
        while (left < size && right < size && offset < size) {
            int comparison = values.get((left + offset) % size)
                    .compareTo(values.get((right + offset) % size));
            if (comparison == 0) {
                offset++;
            } else if (comparison < 0) {
                right += offset + 1;
                if (right == left) {
                    right++;
                }
                offset = 0;
            } else {
                left += offset + 1;
                if (left == right) {
                    left++;
                }
                offset = 0;
            }
        }
        int start = Math.min(left, right);
        List<CoordinateBits> result = new ArrayList<>(size);
        result.addAll(values.subList(start, size));
        result.addAll(values.subList(0, start));
        return result;
    }

    private static int compare(List<CoordinateBits> left, List<CoordinateBits> right) {
        int size = Math.min(left.size(), right.size());
        for (int index = 0; index < size; index++) {
            int comparison = left.get(index).compareTo(right.get(index));
            if (comparison != 0) {
                return comparison;
            }
        }
        return Integer.compare(left.size(), right.size());
    }

    private static List<Map<String, String>> maps(List<CoordinateBits> values) {
        List<Map<String, String>> result = new ArrayList<>(values.size());
        for (CoordinateBits value : values) {
            Map<String, String> coordinate = new LinkedHashMap<>();
            coordinate.put("x", value.x());
            coordinate.put("y", value.y());
            result.add(coordinate);
        }
        return result;
    }

    private static String compactUnchecked(Object value) {
        try {
            return JSON.writeValueAsString(value);
        } catch (Exception error) {
            throw new IllegalStateException(error);
        }
    }

    private record CoordinateBits(String x, String y) implements Comparable<CoordinateBits> {
        private static CoordinateBits of(Coordinate coordinate) {
            return new CoordinateBits(bits(coordinate.x), bits(coordinate.y));
        }

        private static String bits(double value) {
            long bits = Double.doubleToRawLongBits(value == 0.0 ? 0.0 : value);
            return "0x" + HexFormat.of().toHexDigits(bits);
        }

        @Override
        public int compareTo(CoordinateBits other) {
            int comparison = x.compareTo(other.x);
            return comparison != 0 ? comparison : y.compareTo(other.y);
        }

        @Override
        public String toString() {
            return x + "," + y;
        }
    }
}
