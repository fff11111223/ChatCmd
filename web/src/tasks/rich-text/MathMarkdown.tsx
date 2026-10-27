import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import { MarkdownContent } from './MarkdownContent';
import 'katex/dist/katex.min.css';

export default function MathMarkdown({ content, vi, onRunToolCall }: { content: string; vi: boolean; onRunToolCall?: (json: string) => void }) {
  // Sanitization runs first in MarkdownContent. Only the trusted, non-trusting
  // KaTeX renderer may add MathML/styles afterward; user HTML cannot do so.
  return <MarkdownContent content={content} vi={vi} onRunToolCall={onRunToolCall} extraRemark={[remarkMath]} extraRehype={[
    [rehypeKatex, { trust: false, strict: 'warn', maxSize: 20, maxExpand: 1000 }],
  ]} />;
}
