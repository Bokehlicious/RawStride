# Generates crates/pw-metadata/src/iptc_table.rs from the IPTC Photo Metadata TechReference JSON
# (ADR 0007 section 3, T76). Windows PowerShell 5.1, no modules.
#
#   .\tools\iptc-table.ps1 -Json D:\PhotoWorkflowData\references\iptc-pmd-techreference_2025.1.json
#
# The same JSON always gives byte-identical output, already in rustfmt layout. A datatype/dataformat
# combination, propoccurrence or ipmdschema that the Rust types cannot express stops the script
# before anything is written.
param(
    [Parameter(Mandatory)][string]$Json,
    [string]$Out
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrEmpty($Out)) {
    $Out = Join-Path $repositoryRoot 'crates\pw-metadata\src\iptc_table.rs'
}
$maxWidth = 100

function Stop-Table {
    param([string]$Message)
    throw "iptc-table: $Message. Nothing was written."
}

function Get-Key {
    # The value of a JSON key, or $null when the key is missing.
    param($Object, [string]$Name)
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
}

function Get-RequiredKey {
    param($Object, [string]$Name, [string]$Where)
    $value = Get-Key $Object $Name
    if ($null -eq $value) { Stop-Table "$Where has no '$Name'" }
    return [string]$value
}

function ConvertTo-RustString {
    param([string]$Text)
    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append('"')
    foreach ($character in $Text.ToCharArray()) {
        $code = [int]$character
        if ($character -eq '\') { [void]$builder.Append('\\') }
        elseif ($character -eq '"') { [void]$builder.Append('\"') }
        elseif ($code -eq 10) { [void]$builder.Append('\n') }
        elseif ($code -eq 13) { [void]$builder.Append('\r') }
        elseif ($code -eq 9) { [void]$builder.Append('\t') }
        elseif ($code -lt 32 -or $code -eq 127) { [void]$builder.Append(('\u{{{0:x}}}' -f $code)) }
        else { [void]$builder.Append($character) }
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function ConvertTo-RustOption {
    param($Value)
    if ($null -eq $Value) { return 'None' }
    return 'Some(' + (ConvertTo-RustString ([string]$Value)) + ')'
}

function Get-ValueKind {
    param($Entry, [string]$Where, $StructNames)
    $datatype = Get-RequiredKey $Entry 'datatype' $Where
    $dataformat = Get-Key $Entry 'dataformat'
    if ($datatype -ceq 'string') {
        if ($null -eq $dataformat) { return 'ValueKind::Text' }
        if ($dataformat -ceq 'date-time') { return 'ValueKind::Date' }
        if ($dataformat -ceq 'uri') { return 'ValueKind::Uri' }
        if ($dataformat -ceq 'url') { return 'ValueKind::Url' }
    }
    elseif ($datatype -ceq 'number') {
        if ($null -eq $dataformat) { return 'ValueKind::Number' }
        if ($dataformat -ceq 'integer') { return 'ValueKind::Integer' }
    }
    elseif ($datatype -ceq 'any') {
        if ($null -eq $dataformat) { return 'ValueKind::Any' }
    }
    elseif ($datatype -ceq 'struct') {
        if ($dataformat -ceq 'AltLang') { return 'ValueKind::LangAlt' }
        if ($null -ne $dataformat -and $StructNames -ccontains [string]$dataformat) {
            return 'ValueKind::Struct(' + (ConvertTo-RustString ([string]$dataformat)) + ')'
        }
    }
    Stop-Table "$Where has datatype '$datatype' with dataformat '$dataformat', which has no ValueKind"
}

function Get-Occurrence {
    param($Entry, [string]$Where)
    $occurrence = Get-RequiredKey $Entry 'propoccurrence' $Where
    if ($occurrence -ceq 'single') { return 'Occurrence::Single' }
    if ($occurrence -ceq 'multi') { return 'Occurrence::Multi' }
    Stop-Table "$Where has the unknown propoccurrence '$occurrence'"
}

function Get-Schema {
    param($Entry, [string]$Where)
    $schema = Get-RequiredKey $Entry 'ipmdschema' $Where
    if ($schema -ceq 'IptcCore') { return 'Schema::Core' }
    if ($schema -ceq 'IptcExt') { return 'Schema::Extension' }
    Stop-Table "$Where has the unknown ipmdschema '$schema'"
}

function Get-SortedEntries {
    # The entries of a JSON object as (Key, Value) pairs, ordered by 'sortorder' (ordinal), then by
    # key, so the output never depends on the order of the JSON or on the culture.
    param($Object, [string]$Where)
    $list = New-Object 'System.Collections.Generic.List[object]'
    foreach ($property in $Object.PSObject.Properties) {
        $sortOrder = Get-RequiredKey $property.Value 'sortorder' "$Where '$($property.Name)'"
        $list.Add([pscustomobject]@{ Key = $property.Name; Value = $property.Value; Sort = $sortOrder })
    }
    $comparison = [Comparison[object]] {
        param($left, $right)
        $result = [string]::CompareOrdinal($left.Sort, $right.Sort)
        if ($result -eq 0) { $result = [string]::CompareOrdinal($left.Key, $right.Key) }
        return $result
    }
    $list.Sort($comparison)
    return , $list
}

$lines = New-Object 'System.Collections.Generic.List[string]'

function Add-Field {
    # One `name: value,` line, wrapped the way rustfmt wraps a single argument that is too long.
    param([int]$Indent, [string]$Name, [string]$Value)
    $pad = ' ' * $Indent
    $line = "${pad}${Name}: $Value,"
    if ($line.Length -le $maxWidth) {
        $lines.Add($line)
        return
    }
    if ($Value -match '^Some\((".*")\)$') {
        $lines.Add("${pad}${Name}: Some(")
        $lines.Add("$pad    $($Matches[1]),")
        $lines.Add("$pad),")
        return
    }
    $lines.Add($line)
}

function Add-Iim {
    param([int]$Indent, $Entry, [string]$Where)
    $iimId = Get-Key $Entry 'IIMid'
    $maxBytes = Get-Key $Entry 'IIMmaxbytes'
    if ($null -eq $iimId) {
        if ($null -ne $maxBytes) { Stop-Table "$Where has IIMmaxbytes but no IIMid" }
        Add-Field $Indent 'iim' 'None'
        return
    }
    if ([string]$iimId -notmatch '^(\d{1,3}):(\d{1,3})$') {
        Stop-Table "$Where has the IIMid '$iimId', which is not <record>:<dataset>"
    }
    $record = [int]$Matches[1]
    $dataset = [int]$Matches[2]
    if ($record -gt 255 -or $dataset -gt 255) { Stop-Table "$Where has the IIMid '$iimId' outside u8" }
    $maxText = 'None'
    if ($null -ne $maxBytes) {
        $maxNumber = 0
        if (-not [int]::TryParse([string]$maxBytes, [ref]$maxNumber) -or $maxNumber -lt 0 -or $maxNumber -gt 65535) {
            Stop-Table "$Where has the IIMmaxbytes '$maxBytes', which is not a u16"
        }
        $maxText = "Some($maxNumber)"
    }
    $pad = ' ' * $Indent
    $lines.Add("${pad}iim: Some(IimDataset {")
    $lines.Add("$pad    record: $record,")
    $lines.Add("$pad    dataset: $dataset,")
    $lines.Add("$pad    max_bytes: $maxText,")
    $lines.Add("$pad}),")
}

# --- Read -------------------------------------------------------------------------------------

$jsonPath = (Resolve-Path -LiteralPath $Json).Path
$jsonText = [System.IO.File]::ReadAllText($jsonPath, [System.Text.Encoding]::UTF8)
$document = ConvertFrom-Json -InputObject $jsonText
$hash = (Get-FileHash -LiteralPath $jsonPath -Algorithm SHA256).Hash.ToLowerInvariant()
$fileName = [System.IO.Path]::GetFileName($jsonPath)
$releaseTimestamp = Get-RequiredKey $document 'release_timestamp' 'The JSON'
$top = Get-Key $document 'ipmd_top'
$structs = Get-Key $document 'ipmd_struct'
if ($null -eq $top) { Stop-Table "The JSON has no 'ipmd_top'" }
if ($null -eq $structs) { Stop-Table "The JSON has no 'ipmd_struct'" }

# 'AltLang' describes the Lang Alt type (its fields have no datatype); it is not a structure.
$structNames = New-Object 'System.Collections.Generic.List[string]'
foreach ($property in $structs.PSObject.Properties) {
    if ($property.Name -cne 'AltLang') { $structNames.Add($property.Name) }
}
$structNames.Sort([System.StringComparer]::Ordinal)

# --- Write ------------------------------------------------------------------------------------

$lines.Add("//! The IPTC property table (ADR 0007 $([char]0x00A7)3).")
$lines.Add('//!')
$lines.Add("//! Generated by ``tools/iptc-table.ps1`` from ``$fileName``")
$lines.Add("//! (release_timestamp $releaseTimestamp),")
$lines.Add("//! SHA-256 $hash.")
$lines.Add('//!')
$lines.Add('//! Do not edit: run the script again.')
$lines.Add('')
$lines.Add('use crate::properties::{')
$lines.Add('    FieldDef, IimDataset, Occurrence, PropertyDef, Schema, StructDef, ValueKind,')
$lines.Add('};')
$lines.Add('')
$lines.Add('pub(crate) static TOP: &[PropertyDef] = &[')
foreach ($item in (Get-SortedEntries $top 'ipmd_top')) {
    $entry = $item.Value
    $where = "ipmd_top '$($item.Key)'"
    $lines.Add('    PropertyDef {')
    Add-Field 8 'key' (ConvertTo-RustString $item.Key)
    Add-Field 8 'name' (ConvertTo-RustString (Get-RequiredKey $entry 'name' $where))
    Add-Field 8 'schema' (Get-Schema $entry $where)
    Add-Field 8 'kind' (Get-ValueKind $entry $where $structNames)
    Add-Field 8 'occurrence' (Get-Occurrence $entry $where)
    Add-Field 8 'xmp_id' (ConvertTo-RustString (Get-RequiredKey $entry 'XMPid' $where))
    Add-Iim 8 $entry $where
    Add-Field 8 'iim_name' (ConvertTo-RustOption (Get-Key $entry 'IIMname'))
    Add-Field 8 'exif_id' (ConvertTo-RustOption (Get-Key $entry 'EXIFid'))
    Add-Field 8 'et_xmp' (ConvertTo-RustOption (Get-Key $entry 'etXMP'))
    Add-Field 8 'et_iim' (ConvertTo-RustOption (Get-Key $entry 'etIIM'))
    Add-Field 8 'et_exif' (ConvertTo-RustOption (Get-Key $entry 'etEXIF'))
    $lines.Add('    },')
}
$lines.Add('];')
$lines.Add('')
$lines.Add('pub(crate) static STRUCTS: &[StructDef] = &[')
foreach ($structName in $structNames) {
    $lines.Add('    StructDef {')
    Add-Field 8 'name' (ConvertTo-RustString $structName)
    $lines.Add('        fields: &[')
    foreach ($item in (Get-SortedEntries (Get-Key $structs $structName) "ipmd_struct '$structName'")) {
        $entry = $item.Value
        $where = "ipmd_struct '$structName' field '$($item.Key)'"
        $lines.Add('            FieldDef {')
        Add-Field 16 'key' (ConvertTo-RustString $item.Key)
        Add-Field 16 'name' (ConvertTo-RustString (Get-RequiredKey $entry 'name' $where))
        Add-Field 16 'kind' (Get-ValueKind $entry $where $structNames)
        Add-Field 16 'occurrence' (Get-Occurrence $entry $where)
        Add-Field 16 'xmp_id' (ConvertTo-RustString (Get-RequiredKey $entry 'XMPid' $where))
        Add-Field 16 'et_tag' (ConvertTo-RustOption (Get-Key $entry 'etTag'))
        $lines.Add('            },')
    }
    $lines.Add('        ],')
    $lines.Add('    },')
}
$lines.Add('];')

$text = ($lines -join "`n") + "`n"
$outPath = [System.IO.Path]::GetFullPath($Out)
[System.IO.File]::WriteAllText($outPath, $text, (New-Object System.Text.UTF8Encoding($false)))
Write-Output "Wrote $outPath ($($lines.Count) lines) from $fileName, SHA-256 $hash"
