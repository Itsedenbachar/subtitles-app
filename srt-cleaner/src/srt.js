window.cleanSRT = function (rawText) {
  // 1. Strip UTF-8 BOM
  if (rawText.charCodeAt(0) === 0xfeff) {
    rawText = rawText.slice(1);
  }

  // 2. Normalise line endings to LF
  rawText = rawText.replace(/\r\n/g, '\n').replace(/\r/g, '\n');

  // 3. Trim leading/trailing whitespace
  rawText = rawText.trim();

  // 4. Split into blocks separated by one or more blank lines
  const blocks = rawText.split(/\n{2,}/);

  const cleanedBlocks = [];
  let sequenceNumber = 1;

  for (const block of blocks) {
    const lines = block.trim().split('\n');

    if (lines.length < 2) continue;

    // 5a. First line is the sequence number (we'll re-index it later)
    // Find the timestamp line — it's the first line matching the SRT timestamp pattern
    let timestampIndex = -1;
    for (let i = 0; i < lines.length; i++) {
      if (/^\d{2}:\d{2}:\d{2}[,\.]\d{3}\s*-->\s*\d{2}:\d{2}:\d{2}[,\.]\d{3}/.test(lines[i])) {
        timestampIndex = i;
        break;
      }
    }

    if (timestampIndex === -1) continue;

    // 5b. Fix timestamp: replace period before milliseconds with a comma
    const fixedTimestamp = lines[timestampIndex].replace(
      /(\d{2}:\d{2}:\d{2})\.(\d{3})/g,
      '$1,$2'
    );

    // 5c. Subtitle text is everything after the timestamp line
    const subtitleLines = lines.slice(timestampIndex + 1);
    if (subtitleLines.length === 0 || subtitleLines.every(l => l.trim() === '')) continue;

    // 6. Re-index sequence number
    const reindexedBlock = [
      String(sequenceNumber),
      fixedTimestamp,
      ...subtitleLines,
    ].join('\n');

    cleanedBlocks.push(reindexedBlock);
    sequenceNumber++;
  }

  // 7 & 8. Join with exactly one blank line, end with single trailing newline
  return cleanedBlocks.join('\n\n') + '\n';
};

window.validateSRT = function (text) {
  if (!text || text.trim() === '') {
    return { valid: false, error: 'File is empty.' };
  }

  // Normalise for validation
  const normalised = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n').trim();
  const blocks = normalised.split(/\n{2,}/);

  if (blocks.length === 0) {
    return { valid: false, error: 'No subtitle blocks found.' };
  }

  const timestampRe = /^(\d{2}):(\d{2}):(\d{2})[,\.](\d{3})\s*-->\s*(\d{2}):(\d{2}):(\d{2})[,\.](\d{3})/;

  for (let i = 0; i < blocks.length; i++) {
    const lines = blocks[i].trim().split('\n');

    if (lines.length < 3) {
      return {
        valid: false,
        error: `Block ${i + 1} is too short — needs a number, a timestamp, and at least one line of text.`,
      };
    }

    // Find timestamp line
    let timestampIndex = -1;
    for (let j = 0; j < lines.length; j++) {
      if (timestampRe.test(lines[j])) {
        timestampIndex = j;
        break;
      }
    }

    if (timestampIndex === -1) {
      return {
        valid: false,
        error: `Block ${i + 1} has no valid timestamp line.`,
      };
    }

    const match = timestampRe.exec(lines[timestampIndex]);

    // Parse start and end times into milliseconds
    const toMs = (h, m, s, ms) =>
      parseInt(h) * 3600000 + parseInt(m) * 60000 + parseInt(s) * 1000 + parseInt(ms);

    const start = toMs(match[1], match[2], match[3], match[4]);
    const end = toMs(match[5], match[6], match[7], match[8]);

    if (end <= start) {
      return {
        valid: false,
        error: `Block ${i + 1} has an end time that is not after the start time.`,
      };
    }

    // Check subtitle text is not empty
    const textLines = lines.slice(timestampIndex + 1);
    if (textLines.length === 0 || textLines.every(l => l.trim() === '')) {
      return {
        valid: false,
        error: `Block ${i + 1} has no subtitle text.`,
      };
    }
  }

  return { valid: true };
};
