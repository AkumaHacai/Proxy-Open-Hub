using System.Text;
using System.Text.RegularExpressions;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Core.Deeplinks;

public enum DeeplinkParseErrorCode
{
    InvalidScheme,
    MissingPayload,
    CorruptedPayload,
    MissingAddress,
    MissingHostname,
    MissingCredentials,
    UnsupportedVersion
}

public sealed class DeeplinkParseException : Exception
{
    public DeeplinkParseException(DeeplinkParseErrorCode code, string message) : base(message)
    {
        Code = code;
    }

    public DeeplinkParseErrorCode Code { get; }
}

public sealed partial class DeeplinkParser
{
    private const ulong MaxSupportedVersion = 1;

    public ImportedProfile Parse(string link)
    {
        if (!link.StartsWith("tt://", StringComparison.OrdinalIgnoreCase))
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.InvalidScheme, "Некорректная ссылка TrustTunnel.");
        }

        var question = link.IndexOf('?');
        var payload = question >= 0 ? link[(question + 1)..].Trim() : link["tt://".Length..].Trim();
        if (string.IsNullOrWhiteSpace(payload))
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.MissingPayload, "Не найден payload TrustTunnel.");
        }

        var bytes = DecodePayload(payload);
        var config = DecodeFields(bytes);

        if (config.Addresses.Count == 0)
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.MissingAddress, "Не найден адрес сервера.");
        }

        if (string.IsNullOrWhiteSpace(config.Hostname))
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.MissingHostname, "Не найден hostname для TLS.");
        }

        if (string.IsNullOrWhiteSpace(config.Username) || string.IsNullOrWhiteSpace(config.Password))
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.MissingCredentials, "Не найдены учётные данные.");
        }

        var profile = new ServerProfile
        {
            DisplayName = string.IsNullOrWhiteSpace(config.Name) ? config.Hostname : config.Name,
            Endpoint = new EndpointConfig
            {
                Hostname = config.Hostname,
                CustomSni = config.CustomSni,
                Addresses = config.Addresses.ToArray(),
                HasIpv6 = config.HasIpv6,
                Username = config.Username,
                ClientRandomSecretRef = "",
                SkipVerification = config.SkipVerification,
                CertificatePem = config.CertificatePem,
                UpstreamProtocol = config.UpstreamProtocol,
                AntiDpi = config.AntiDpi,
                DnsUpstreams = config.DnsUpstreams.ToArray()
            },
            Listener = new ListenerConfig { Mode = ListenerMode.Tun }
        };

        return new ImportedProfile(profile, new SecretCandidate(config.Password, config.ClientRandom), Array.Empty<string>());
    }

    private static byte[] DecodePayload(string payload)
    {
        try
        {
            var normalized = payload.Replace('-', '+').Replace('_', '/');
            normalized = normalized.PadRight(normalized.Length + (4 - normalized.Length % 4) % 4, '=');
            return Convert.FromBase64String(normalized);
        }
        catch (FormatException ex)
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.CorruptedPayload, "Ссылка повреждена или обрезана.") { Source = ex.Source };
        }
    }

    private static DecodedDeepLink DecodeFields(byte[] bytes)
    {
        var config = new DecodedDeepLink();
        var offset = 0;

        while (offset < bytes.Length)
        {
            var tag = ReadVarInt(bytes, ref offset);
            var length = ReadVarInt(bytes, ref offset);
            if (length > int.MaxValue || offset + (int)length > bytes.Length)
            {
                throw new DeeplinkParseException(DeeplinkParseErrorCode.CorruptedPayload, "Ссылка повреждена или обрезана.");
            }

            var value = bytes.AsSpan(offset, (int)length).ToArray();
            offset += (int)length;

            switch (tag)
            {
                case 0x00:
                    config.Version = ReadVarInt(value, 0);
                    if (config.Version > MaxSupportedVersion)
                    {
                        throw new DeeplinkParseException(DeeplinkParseErrorCode.UnsupportedVersion, "Неподдерживаемая версия формата TrustTunnel.");
                    }

                    break;

                case 0x01:
                    config.Hostname = Utf8(value);
                    break;

                case 0x02:
                    config.Addresses.Add(Utf8(value));
                    break;

                case 0x03:
                    config.CustomSni = Utf8(value);
                    break;

                case 0x04:
                    config.HasIpv6 = ReadBool(value);
                    break;

                case 0x05:
                    config.Username = Utf8(value);
                    break;

                case 0x06:
                    config.Password = Utf8(value);
                    break;

                case 0x07:
                    config.SkipVerification = ReadBool(value);
                    break;

                case 0x08:
                    config.CertificatePem = DerCertificatesToPem(value);
                    break;

                case 0x09:
                    config.UpstreamProtocol = ReadVarInt(value, 0) == 0x02 ? UpstreamProtocol.Http3 : UpstreamProtocol.Http2;
                    break;

                case 0x0A:
                    config.AntiDpi = ReadBool(value);
                    break;

                case 0x0B:
                    config.ClientRandom = Utf8(value);
                    break;

                case 0x0C:
                    config.Name = Utf8(value);
                    break;

                case 0x0D:
                    config.DnsUpstreams = ReadStringArray(value);
                    break;
            }
        }

        return config;
    }

    private static ulong ReadVarInt(byte[] bytes, int start)
    {
        var offset = start;
        return ReadVarInt(bytes, ref offset);
    }

    private static ulong ReadVarInt(byte[] bytes, ref int offset)
    {
        if (offset >= bytes.Length)
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.CorruptedPayload, "Ссылка повреждена или обрезана.");
        }

        var first = bytes[offset++];
        var prefix = first >> 6;
        var size = 1 << prefix;
        if (offset + size - 1 > bytes.Length)
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.CorruptedPayload, "Ссылка повреждена или обрезана.");
        }

        var value = (ulong)(first & 0x3F);
        for (var i = 1; i < size; i++)
        {
            value = (value << 8) | bytes[offset++];
        }

        return value;
    }

    private static string Utf8(byte[] value) => Encoding.UTF8.GetString(value);

    private static bool ReadBool(byte[] value)
    {
        if (value.Length != 1)
        {
            throw new DeeplinkParseException(DeeplinkParseErrorCode.CorruptedPayload, "Некорректное bool-поле в TrustTunnel ссылке.");
        }

        return value[0] != 0;
    }

    private static IReadOnlyList<string> ReadStringArray(byte[] value)
    {
        var items = new List<string>();
        var offset = 0;
        while (offset < value.Length)
        {
            var length = ReadVarInt(value, ref offset);
            if (length > int.MaxValue || offset + (int)length > value.Length)
            {
                throw new DeeplinkParseException(DeeplinkParseErrorCode.CorruptedPayload, "Некорректный список строк в TrustTunnel ссылке.");
            }

            items.Add(Encoding.UTF8.GetString(value, offset, (int)length));
            offset += (int)length;
        }

        return items;
    }

    private static string DerCertificatesToPem(byte[] value)
    {
        if (value.Length == 0)
        {
            return "";
        }

        var blocks = SplitDerCertificates(value).ToArray();
        if (blocks.Length == 0)
        {
            blocks = new[] { value };
        }

        var builder = new StringBuilder();
        foreach (var block in blocks)
        {
            builder.AppendLine("-----BEGIN CERTIFICATE-----");
            builder.AppendLine(Convert.ToBase64String(block, Base64FormattingOptions.InsertLineBreaks));
            builder.AppendLine("-----END CERTIFICATE-----");
        }

        return builder.ToString().TrimEnd();
    }

    private static IEnumerable<byte[]> SplitDerCertificates(byte[] value)
    {
        var offset = 0;
        while (offset < value.Length)
        {
            if (value[offset] != 0x30)
            {
                yield break;
            }

            var start = offset++;
            if (offset >= value.Length)
            {
                yield break;
            }

            var lengthByte = value[offset++];
            int contentLength;
            if ((lengthByte & 0x80) == 0)
            {
                contentLength = lengthByte;
            }
            else
            {
                var lengthBytes = lengthByte & 0x7F;
                if (lengthBytes is 0 or > 4 || offset + lengthBytes > value.Length)
                {
                    yield break;
                }

                contentLength = 0;
                for (var i = 0; i < lengthBytes; i++)
                {
                    contentLength = (contentLength << 8) | value[offset++];
                }
            }

            var totalLength = offset - start + contentLength;
            if (contentLength < 0 || start + totalLength > value.Length)
            {
                yield break;
            }

            yield return value[start..(start + totalLength)];
            offset = start + totalLength;
        }
    }

    private sealed class DecodedDeepLink
    {
        public ulong Version { get; set; }
        public string Hostname { get; set; } = "";
        public List<string> Addresses { get; } = new();
        public string CustomSni { get; set; } = "";
        public bool HasIpv6 { get; set; } = true;
        public string Username { get; set; } = "";
        public string Password { get; set; } = "";
        public bool SkipVerification { get; set; }
        public string CertificatePem { get; set; } = "";
        public UpstreamProtocol UpstreamProtocol { get; set; } = UpstreamProtocol.Http2;
        public bool AntiDpi { get; set; }
        public string ClientRandom { get; set; } = "";
        public string Name { get; set; } = "";
        public IReadOnlyList<string> DnsUpstreams { get; set; } = Array.Empty<string>();
    }
}
