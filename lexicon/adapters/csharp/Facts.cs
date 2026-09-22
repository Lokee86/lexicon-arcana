using System.Security.Cryptography;
using System.Text;

internal static class Facts
{
    internal const string Language = "csharp";

    internal static string NodeId(string kind, string canonicalIdentity)
    {
        return Digest($"lexicon:v1\0{Language}\0{kind}\0{canonicalIdentity}");
    }

    internal static string ContentId(byte[] content)
    {
        return ContentId((ReadOnlySpan<byte>)content);
    }

    internal static string ContentId(ReadOnlySpan<byte> content)
    {
        return FormatDigest(SHA256.HashData(content));
    }

    internal static string NormalizePath(string path)
    {
        return path.Replace('\\', '/');
    }

    private static string Digest(string value)
    {
        return FormatDigest(SHA256.HashData(Encoding.UTF8.GetBytes(value)));
    }

    private static string FormatDigest(byte[] digest)
    {
        return $"sha256:{Convert.ToHexString(digest).ToLowerInvariant()}";
    }
}

internal sealed class FactStore
{
    private readonly Dictionary<string, NodeRecord> nodes = new(StringComparer.Ordinal);
    private readonly Dictionary<string, EdgeRecord> edges = new(StringComparer.Ordinal);
    private readonly Dictionary<string, UnresolvedRecord> unresolved = new(StringComparer.Ordinal);

    internal IReadOnlyCollection<NodeRecord> Nodes => nodes.Values;
    internal IReadOnlyCollection<EdgeRecord> Edges => edges.Values;
    internal IReadOnlyCollection<UnresolvedRecord> Unresolved => unresolved.Values;

    internal string AddNode(NodeRecord record)
    {
        ArgumentNullException.ThrowIfNull(record);
        if (nodes.TryGetValue(record.Id, out var existing))
        {
            nodes[record.Id] = CanonicalNode(existing, record);
            return record.Id;
        }

        nodes.Add(record.Id, record);
        return record.Id;
    }


    private static NodeRecord CanonicalNode(NodeRecord left, NodeRecord right)
    {
        var leftExternal = string.Equals(left.Path, "external", StringComparison.Ordinal);
        var rightExternal = string.Equals(right.Path, "external", StringComparison.Ordinal);
        if (leftExternal != rightExternal)
        {
            return leftExternal ? right : left;
        }

        var leftOwned = !string.IsNullOrEmpty(left.Owner);
        var rightOwned = !string.IsNullOrEmpty(right.Owner);
        if (leftOwned != rightOwned)
        {
            return leftOwned ? left : right;
        }

        return string.CompareOrdinal(Jsonl.SerializeLine(left), Jsonl.SerializeLine(right)) <= 0 ? left : right;
    }

    internal string AddNode(
        string kind,
        string name,
        string path,
        string qualifiedName,
        string? identity = null,
        SpanRecord? span = null,
        IReadOnlyDictionary<string, object?>? attributes = null,
        string? contentId = null,
        string? owner = null)
    {
        var id = Facts.NodeId(kind, identity ?? qualifiedName);
        return AddNode(new NodeRecord
        {
            Attributes = attributes,
            ContentId = contentId,
            Id = id,
            Kind = kind,
            Name = name,
            Owner = owner,
            Path = path,
            QualifiedName = qualifiedName,
            Record = "node",
            Span = span,
        });
    }

    internal void AddEdge(EdgeRecord record)
    {
        ArgumentNullException.ThrowIfNull(record);
        edges.TryAdd(EdgeKey(record), record);
    }

    internal void AddUnresolved(UnresolvedRecord record)
    {
        ArgumentNullException.ThrowIfNull(record);
        unresolved.TryAdd(UnresolvedKey(record), record);
    }

    private static string EdgeKey(EdgeRecord record)
    {
        return string.Join("\0",
            record.Source,
            record.Target,
            record.Relation,
            NormalizeOwner(record.Owner) ?? string.Empty,
            SpanKey(record.Span),
            AttributesKey(record.Attributes));
    }

    private static string UnresolvedKey(UnresolvedRecord record)
    {
        return string.Join("\0",
            record.Source,
            record.Relation,
            record.Expression,
            record.Reason,
            record.CandidateName ?? string.Empty,
            record.CandidateNamespace ?? string.Empty,
            NormalizeOwner(record.Owner) ?? string.Empty,
            SpanKey(record.Span),
            AttributesKey(record.Attributes));
    }

    private static string SpanKey(SpanRecord? span)
    {
        return span is null
            ? string.Empty
            : string.Join("\0",
                span.Path,
                span.StartLine.ToString(System.Globalization.CultureInfo.InvariantCulture),
                span.StartColumn.ToString(System.Globalization.CultureInfo.InvariantCulture),
                span.EndLine.ToString(System.Globalization.CultureInfo.InvariantCulture),
                span.EndColumn.ToString(System.Globalization.CultureInfo.InvariantCulture));
    }

    private static string AttributesKey(IReadOnlyDictionary<string, object?>? attributes)
    {
        return attributes is null || attributes.Count == 0
            ? string.Empty
            : Jsonl.SerializeLine(attributes);
    }

    internal string EmitJsonl(HeaderRecord header)
    {
        ArgumentNullException.ThrowIfNull(header);
        var incremental = string.Equals(header.Mode, "incremental", StringComparison.Ordinal);
        var changedFiles = NormalizePaths(header.ChangedFiles);
        var removedFiles = NormalizePaths(header.RemovedFiles);
        if (incremental && (header.ChangedFiles is null || header.RemovedFiles is null))
        {
            throw new ArgumentException("incremental headers require changed_files and removed_files");
        }

        if (changedFiles.Intersect(removedFiles, StringComparer.Ordinal).Any())
        {
            throw new ArgumentException("changed_files and removed_files must be disjoint");
        }

        var canonicalHeader = new HeaderRecord
        {
            AdapterVersion = header.AdapterVersion,
            ChangedFiles = header.ChangedFiles is null ? null : changedFiles,
            Language = header.Language,
            Mode = header.Mode,
            Record = header.Record,
            RemovedFiles = header.RemovedFiles is null ? null : removedFiles,
            Repository = header.Repository,
            SchemaVersion = header.SchemaVersion,
            SharedComplete = header.SharedComplete,
        };
        var owners = nodes.Values.ToDictionary(node => node.Id, DirectOwner, StringComparer.Ordinal);
        var selected = changedFiles.ToHashSet(StringComparer.Ordinal);
        var output = new StringBuilder();
        output.Append(Jsonl.SerializeLine(canonicalHeader)).Append('\n');

        foreach (var node in OrderedNodes())
        {
            var owner = DirectOwner(node);
            if (!incremental || Include(owner, selected) || IncludeShared(owner, canonicalHeader))
            {
                output.Append(Jsonl.SerializeLine(node)).Append('\n');
            }
        }

        foreach (var edge in OrderedEdges())
        {
            var owner = DirectOwner(edge);
            if (owner.Length == 0 && owners.TryGetValue(edge.Source, out var sourceOwner))
            {
                owner = sourceOwner;
            }

            if (!incremental || Include(owner, selected) || IncludeShared(owner, canonicalHeader))
            {
                output.Append(Jsonl.SerializeLine(edge)).Append('\n');
            }
        }

        foreach (var record in OrderedUnresolved())
        {
            var owner = DirectOwner(record);
            if (owner.Length == 0 && owners.TryGetValue(record.Source, out var sourceOwner))
            {
                owner = sourceOwner;
            }

            if (!incremental || Include(owner, selected) || IncludeShared(owner, canonicalHeader))
            {
                output.Append(Jsonl.SerializeLine(record)).Append('\n');
            }
        }

        return output.ToString();
    }

    private IEnumerable<NodeRecord> OrderedNodes()
    {
        return nodes.Values
            .OrderBy(node => node.Id, StringComparer.Ordinal)
            .ThenBy(node => node.Kind, StringComparer.Ordinal)
            .ThenBy(node => node.Path, StringComparer.Ordinal)
            .ThenBy(node => node.QualifiedName, StringComparer.Ordinal)
            .ThenBy(node => Jsonl.SerializeLine(node), StringComparer.Ordinal);
    }

    private IEnumerable<EdgeRecord> OrderedEdges()
    {
        return edges.Values
            .OrderBy(edge => edge.Source, StringComparer.Ordinal)
            .ThenBy(edge => edge.Target, StringComparer.Ordinal)
            .ThenBy(edge => edge.Relation, StringComparer.Ordinal)
            .ThenBy(edge => SpanPath(edge.Span), StringComparer.Ordinal)
            .ThenBy(edge => SpanValue(edge.Span, span => span.StartLine))
            .ThenBy(edge => SpanValue(edge.Span, span => span.StartColumn))
            .ThenBy(edge => SpanValue(edge.Span, span => span.EndLine))
            .ThenBy(edge => SpanValue(edge.Span, span => span.EndColumn))
            .ThenBy(Jsonl.SerializeLine, StringComparer.Ordinal);
    }

    private IEnumerable<UnresolvedRecord> OrderedUnresolved()
    {
        return unresolved.Values
            .OrderBy(record => record.Source, StringComparer.Ordinal)
            .ThenBy(record => record.Relation, StringComparer.Ordinal)
            .ThenBy(record => record.Expression, StringComparer.Ordinal)
            .ThenBy(record => record.Reason, StringComparer.Ordinal)
            .ThenBy(record => SpanPath(record.Span), StringComparer.Ordinal)
            .ThenBy(record => SpanValue(record.Span, span => span.StartLine))
            .ThenBy(record => SpanValue(record.Span, span => span.StartColumn))
            .ThenBy(record => SpanValue(record.Span, span => span.EndLine))
            .ThenBy(record => SpanValue(record.Span, span => span.EndColumn))
            .ThenBy(Jsonl.SerializeLine, StringComparer.Ordinal);
    }

    private static string DirectOwner(NodeRecord record)
    {
        return NormalizeOwner(record.Owner)
            ?? NormalizeOwner(record.Span?.Path)
            ?? (record.Kind == "file" ? Facts.NormalizePath(record.Path) : string.Empty);
    }

    private static string DirectOwner(EdgeRecord record)
    {
        return NormalizeOwner(record.Owner) ?? NormalizeOwner(record.Span?.Path) ?? string.Empty;
    }

    private static string DirectOwner(UnresolvedRecord record)
    {
        return NormalizeOwner(record.Owner) ?? NormalizeOwner(record.Span?.Path) ?? string.Empty;
    }

    private static string? NormalizeOwner(string? owner)
    {
        return string.IsNullOrEmpty(owner) ? null : Facts.NormalizePath(owner);
    }

    private static IReadOnlyList<string> NormalizePaths(IReadOnlyList<string>? paths)
    {
        return (paths ?? Array.Empty<string>())
            .Select(Facts.NormalizePath)
            .Distinct(StringComparer.Ordinal)
            .OrderBy(path => path, StringComparer.Ordinal)
            .ToArray();
    }

    private static bool Include(string owner, IReadOnlySet<string> selected)
    {
        return owner.Length > 0 && selected.Contains(owner);
    }

    private static bool IncludeShared(string owner, HeaderRecord header)
    {
        return owner.Length == 0 && header.SharedComplete == true;
    }

    private static string SpanPath(SpanRecord? span) => span is null ? string.Empty : span.Path;

    private static int SpanValue(SpanRecord? span, Func<SpanRecord, int> selector) => span is null ? 0 : selector(span);
}
