import { useEffect, type ReactNode } from "react";
import { Alert } from "@/registry/components/alert/alert";
import { Badge } from "@/registry/components/badge/badge";
import { Button } from "@/registry/components/button/button";
import { CodeBlock } from "@/registry/components/code-block/code-block";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/registry/components/tabs/tabs";
import ApiTester from "./ApiTester";
import CoinPlayground from "./CoinPlayground";
import { exampleResponses, pretty, primitiveRequest, primitiveTypes, quickstartExamples } from "./data";
import parisii from "./assets/images/parisii.jpg";
import hadrian from "./assets/images/hadrian.jpg";
import "./app.css";

const accuracy = [["2", "50%"], ["3", "33.3%"], ["5", "20%"], ["10", "10%"], ["255", "0.39%"]];
const models = [
  ["flip-latest", "most recent stable release of the coin"],
  ["flip-preview", "resolves to model flip-1.1.0, the two-sided coin"],
  ["flip-edge", "forces an edge landing; uncertainty depends on the decision type"],
  ["flip-overloaded", "returns 529 so you can test retry logic. The coin itself is fine."],
];
const errors = [
  ["401", "Missing bearer"], ["413", "Over 32 MiB"],
  ["415", "Unsupported/double encoding"],
  ["400", "Malformed JSON/BOM/non-finite/depth>64/corrupt compression"],
  ["422", "Field errors"], ["429", "never: the coin cannot be overloaded"],
];

function scrollToSection(id: string) {
  const section = document.getElementById(id);
  section?.scrollIntoView({
    behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth",
    block: "start",
  });
  section?.focus({ preventScroll: true });
}

function Chapter({ id, className = "", children }: { id: string; className?: string; children: ReactNode }) {
  return <section id={id} className={`chapter ${className}`} tabIndex={-1}>{children}</section>;
}

function Hero() {
  return (
    <section className="hero" aria-labelledby="hero-title">
      <div className="hero-copy">
        <h1 id="hero-title">Decisions,<br /><span className="hero-title-accent">by chance.</span></h1>
        <p className="hero-tagline">Every decision model is a coin flip.<br className="desktop-break" /> Flip admits it.</p>
        <div className="hero-actions">
          <Button data-ui type="button" onClick={() => scrollToSection("coin-playground")}>Flip a coin</Button>
          <a href="#quickstart">Read the quickstart</a>
        </div>
        <p className="hero-product">Flip 1.0 · Decision-model API · Zero cost</p>
      </div>
      <div className="hero-stage">
        <div className="hero-specimen-head"><span>THE MODEL</span><span>Au · 1.0</span></div>
        <figure className="hero-object">
          <div className="coin-photo parisii-photo">
            <img src={parisii} width="1600" height="1600" alt="A Celtic gold coin, its face worn into a knowing expression" fetchPriority="high" />
          </div>
          <figcaption>The entire inference stack.<br /><a href="#image-credits">Historical gold, from the museum archives.</a></figcaption>
        </figure>
        <div className="hero-specimen-foot"><span>Zero parameters.</span><span>Two sides.</span></div>
      </div>
    </section>
  );
}

function Benchmarks() {
  return (
    <Chapter id="benchmarks" className="benchmarks">
      <div className="benchmark-copy">
        <h2>Random by design.</h2>
        <p>Zero parameters. Zero training data. The confidence of a coin.</p>
      </div>
      <dl className="benchmark-grid">
        <div><dt>Binary accuracy</dt><dd>50%<small>Exactly random chance</small></dd></div>
        <div><dt>Cost per request</dt><dd>$0.00<small>Usage reported, tokens free</small></dd></div>
        <div><dt>Edge landing</dt><dd>1 / 6000<small>Per question, or on demand</small></dd></div>
      </dl>
    </Chapter>
  );
}

function Accuracy() {
  return (
    <Chapter id="accuracy" className="accuracy">
      <div className="section-heading">
        <h2>Random chance.<br />By design.</h2>
        <div>
          <p>Flip performs at exactly random chance on every question type. Not a limitation: the specification.</p>
          <p>More options. Less accuracy. Exactly as advertised.</p>
        </div>
      </div>
      <table className="accuracy-table">
        <caption className="sr-only">Expected accuracy by number of options</caption>
        <thead><tr><th scope="col">Options</th>{accuracy.map(([options]) => <th scope="col" key={options}>{options}</th>)}</tr></thead>
        <tbody><tr><th scope="row">Accuracy</th>{accuracy.map(([options, percent]) => <td key={options}>{percent}</td>)}</tr></tbody>
      </table>

      <table className="accuracy-mobile">
        <caption className="sr-only">Expected accuracy by number of options</caption>
        <thead><tr><th scope="col">Options</th><th scope="col">Accuracy</th></tr></thead>
        <tbody>{accuracy.map(([options, percent]) => <tr key={options}><th scope="row">{options}</th><td>{percent}</td></tr>)}</tbody>
      </table>
      <p className="section-note">Expected score error is <code>(n<sup>2</sup> − 1) / (3n)</code> levels when prediction and truth are both uniform. With 3 options, that is <code>0.89 levels</code>. Published so our error bar stays honest.</p>
    </Chapter>
  );
}

function Quickstart() {
  return (
    <Chapter id="quickstart" className="quickstart">
      <div className="section-heading">
        <h2>Your first<br />arbitrary decision.</h2>
        <p>One request. One coin.<br />A very confident answer.</p>
      </div>
      <div className="call-response">
        <div data-ui className="request-specimen quickstart-examples">
          <Tabs defaultValue="curl">
            <TabsList aria-label="Quickstart language">
              {quickstartExamples.map((example) => <TabsTrigger key={example.id} value={example.id}>{example.label}</TabsTrigger>)}
            </TabsList>
            {quickstartExamples.map((example) => (
              <TabsContent key={example.id} value={example.id}>
                <p className="example-setup">{example.setup}</p>
                <CodeBlock code={example.code} language={example.language} filename={example.filename} />
              </TabsContent>
            ))}
          </Tabs>
        </div>
        <div className="quickstart-output">
          <div data-ui className="response-specimen"><CodeBlock code={exampleResponses.noul} language="json" filename="Example response · 200 OK" /></div>
          <div className="quickstart-notes">
            <p><code>/api/alpha/decisions</code> is byte-compatible for OpenRouter clients. Keep your client. Change your expectations.</p>
            <p><code>state</code> and <code>instructions</code> accept strings, objects, or arrays. Each question has its own id and type-specific <code>criteria</code>. Authorization requires <code>Bearer</code> followed by any non-empty string.</p>
            <p className="small">Responses shown in the docs are examples. Usage is synthetic: input tokens are decoded request bytes divided by four, rounded up; output tokens count questions.</p>
          </div>
        </div>
      </div>
    </Chapter>
  );
}

const primitiveDescriptions = {
  noul: "Yes or no. The original binary decision model. No confidence field, just the answer.",
  choice: "Pick a department. Or any option. One gets probability 1.0; the others get 0.0.",
  score: "An ordinal score, a legend, and the certainty of a coin that has never read a support ticket.",
  box: "Normalized coordinates for a rectangle. The image is optional to the coin's process.",
};

function Primitives() {
  return (
    <Chapter id="primitives" className="primitives">
      <div className="primitive-cover">
        <h2>Four primitives.<br />One mechanism.</h2>
        <figure className="specimen-plate">
          <div className="coin-photo hadrian-photo"><img src={hadrian} width="1600" height="889" alt="The portrait of Hadrian on a Roman gold aureus" loading="lazy" /></div>
          <figcaption>The wire format you expect.<br />The reasoning you suspect.</figcaption>
        </figure>
      </div>
      <div className="primitive-browser">
        <Tabs defaultValue="noul">
          <div data-ui><TabsList aria-label="Decision primitive examples">
            {primitiveTypes.map((type) => <TabsTrigger key={type} value={type}>{type}</TabsTrigger>)}
          </TabsList></div>
          {primitiveTypes.map((type) => (
            <TabsContent key={type} value={type}>
              <div className="primitive-intro"><p>{primitiveDescriptions[type]}</p>{type === "box" && <Badge>Dreamer-ready</Badge>}</div>
              <div className="primitive-code-stack">
                <div data-ui><CodeBlock code={pretty(primitiveRequest(type))} filename={`${type} request`} language="json" /></div>
                <div data-ui><CodeBlock code={exampleResponses[type]} filename={`${type} example response`} language="json" /></div>
              </div>
              {type === "box" && <p className="section-note">On the edge, we return the whole image. Comprehensive coverage.</p>}
            </TabsContent>
          ))}
        </Tabs>
      </div>
    </Chapter>
  );
}

function Confidence() {
  return (
    <Chapter id="confidence" className="confidence">
      <div className="confidence-intro">
        <h2>How Flip<br />decides</h2>
        <p className="lead">We ignore your prompt with mathematical precision.</p>
        <p>Flip implements TypeSafe's published confidence formulas exactly. Choice and Score report confidence from their probability distributions. Noul reports only its value; Box reports its rectangle and confidence.</p>
      </div>
      <div className="formula-list">
        <div><h3>Choice confidence</h3><code>(p_max - 1/n) / (1 - 1/n)</code></div>
        <div><h3>Score confidence</h3><code>max(0, 1 - spread / MAD_unif)</code></div>
        <p>A one-hot distribution is certain. A confident flip is exactly 1.0. A uniform distribution admits it has no idea.</p>
      </div>
      <div className="edge-landing">
        <p className="edge-figure">1 in 6000</p>
        <div>
          <p>The coin lands on its edge 1 in 6000 times per question. Noul returns <code>0.5</code>. Choice and Score return uniform probabilities, with confidence <code>0.0</code> when there are multiple options; a single option remains certain at <code>1.0</code>. Edge scores are <code>(n-1)/2</code>. Box returns the full image bounds with confidence <code>0.0</code>. Use <code>flip-edge</code> to skip the wait.</p>
          <p>Our confidence measures concentration.<br />Our accuracy measures luck.</p>
        </div>
      </div>
    </Chapter>
  );
}

function Compatibility() {
  return (
    <Chapter id="compatibility" className="compatibility">
      <div className="section-heading">
        <h2>Compatible where<br />it counts.</h2>
        <p>Familiar contracts. A refreshingly underqualified implementation.</p>
      </div>
      <p className="contract-note">TypeSafe <code>POST /v1/systemone</code> and OpenRouter <code>POST /api/alpha/decisions</code>: familiar request and response formats for all four decision types.</p>
      <div className="compatibility-grid">
        <div>
          <h3>Replicated</h3>
          <ul className="feature-list">
            <li>Request/response wire shapes for all four primitives</li>
            <li>TypeSafe's exact confidence math</li>
            <li>Error statuses 400/401/413/415/422</li>
            <li>Alias resolution to <code>flip-1.0.0</code></li>
            <li><code>GET /v1/models</code></li>
            <li>Multipart image envelopes (bytes discarded)</li>
          </ul>
        </div>
        <div>
          <h3>Deliberately different</h3>
          <ul className="feature-list">
            <li>Any bearer token accepted</li><li>Any model id accepted</li>
            <li>Images ignored (the coin doesn't look at your photos)</li><li>No context limits</li>
            <li>Answers are independent. Apply your own consistency checks when combining multiple decisions.</li>
          </ul>
        </div>
      </div>
    </Chapter>
  );
}

function ModelsAndErrors() {
  return (
    <Chapter id="models-and-errors" className="models-errors">
      <h2>Models &amp; errors</h2>
      <div className="reference-stack">
        <div className="model-reference">
          <h3>Choose your coin</h3>
          <p><code>GET /v1/models</code> returns a <code>models</code> array with <code>name</code>, <code>description</code>, and <code>release_date</code>. Bring the same bearer token.</p>
          <table className="data-table">
            <caption className="sr-only">Model aliases and behavior</caption>
            <thead><tr><th scope="col">Model alias</th><th scope="col">Behavior</th></tr></thead>
            <tbody>{models.map(([model, description]) => <tr key={model}><th scope="row"><code>{model}</code></th><td>{description}</td></tr>)}</tbody>
          </table>
          <p className="small">The descriptions above are the model catalog's pitch. Every successful response reports <code>flip-1.0.0</code>, including <code>flip-preview</code> and any other model id. The probabilities have not become wiser.</p>
        </div>
        <div className="error-reference">
          <h3>Failures, faithfully reproduced</h3>
          <table className="data-table error-table">
            <caption className="sr-only">HTTP error statuses and causes</caption>
            <thead><tr><th scope="col">Status</th><th scope="col">Meaning</th></tr></thead>
            <tbody>{errors.map(([status, meaning]) => <tr key={status}><th scope="row"><code>{status}</code></th><td>{meaning}</td></tr>)}</tbody>
          </table>
          <p className="small">The <code>flip-overloaded</code> alias deliberately returns HTTP <code>529</code> for retry testing.</p>
        </div>
      </div>
      <div className="code-pair error-envelopes" data-ui>
        <CodeBlock language="json" filename="TypeSafe error envelope" code={'{\n  "error": {\n    "code": "invalid_request",\n    "message": "..."\n  }\n}'} />
        <CodeBlock language="json" filename="OpenRouter error envelope" code={'{\n  "error": { "code": 422, "message": "..." },\n  "id": "00000000-0000-4000-8000-000000000000"\n}'} />
      </div>
    </Chapter>
  );
}

function Footer() {
  return (
    <footer className="site-footer">
      <div className="footer-action">
        <h2>Leave it to chance.</h2>
        <Button data-ui type="button" size="lg" onClick={() => scrollToSection("coin-playground")}>Flip it</Button>
      </div>
      <details className="image-credits" id="image-credits"><summary>Coin photographs &amp; credits</summary>
        <p>Gold, before it was an inference engine.</p>
        <p><a href="https://www.metmuseum.org/art/collection/search/464811">Gold Coin of the Parisii</a>, Celtic maker, last quarter 2nd century BCE. The Metropolitan Museum of Art. <a href="https://creativecommons.org/publicdomain/zero/1.0/">CC0.</a></p>
        <p><a href="https://www.artic.edu/artworks/182871">Aureus (Coin) Portraying Emperor Hadrian</a>, Ancient Roman, 120–123. Art Institute of Chicago. <a href="https://creativecommons.org/publicdomain/zero/1.0/">CC0.</a> Museum photographs are cropped; they illustrate the coin's history.</p>
      </details>
      <div className="footer-byline"><a className="wordmark" href="#main" aria-label="Flip home">flip<span aria-hidden="true">.</span></a>
        <span className="header-description">Artificial intelligence.<br />Natural uncertainty.</span><p>MIT. Built with LavX LMSUI.</p><p>The coin is fair; we checked.</p><a href="#main">Back to top</a></div>
    </footer>
  );
}

function useReferenceAnchors() {
  useEffect(() => {
    const reveal = () => {
      const target = document.getElementById(window.location.hash.slice(1));
      if (!target) return;
      let ancestor = target.parentElement;
      while (ancestor) {
        if (ancestor instanceof HTMLDetailsElement) ancestor.open = true;
        ancestor = ancestor.parentElement;
      }
      if (target instanceof HTMLDetailsElement) target.open = true;
      target.scrollIntoView({ block: "start" });
    };
    reveal();
    window.addEventListener("hashchange", reveal);
    return () => window.removeEventListener("hashchange", reveal);
  }, []);
}

export default function App() {
  useReferenceAnchors();
  return (
    <>
      <a className="skip-link" href="#main">Skip to content</a>
      <header className="site-header">
        <a className="wordmark" href="#main" aria-label="Flip home">flip<span aria-hidden="true">.</span></a>
        <span className="header-description">Artificial intelligence.<br />Natural uncertainty.</span>
        <nav aria-label="Main navigation"><a href="#coin-playground">Playground</a><a href="#quickstart">Quickstart</a><a href="#reference">Reference</a></nav>
        <Button data-ui type="button" className="header-cta" variant="ghost" onClick={() => scrollToSection("api-tester")}>Try the API<svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M5 12h14M13 6l6 6-6 6" /></svg></Button>
      </header>
      <main id="main" tabIndex={-1}>
        <Hero />
        <section className="coin-chapter chapter" aria-label="Live coin playground"><CoinPlayground /></section>
        <Benchmarks /><Quickstart />
        <section className="tool-chapter chapter" aria-label="Live API tester"><ApiTester /></section>
        <Chapter id="reference" className="reference-chapter">
          <div className="section-heading"><h2>The details, when you need them.</h2><p>Request formats, probabilities, and the fine print.</p></div>
          <details className="reference-disclosure"><summary>Decision types &amp; examples</summary><Primitives /></details>
          <details className="reference-disclosure"><summary>Accuracy &amp; confidence</summary><Accuracy /><Confidence /></details>
          <details className="reference-disclosure"><summary>API compatibility</summary><Compatibility /></details>
          <details className="reference-disclosure"><summary>Models &amp; errors</summary><ModelsAndErrors /></details>
        </Chapter>
        <div className="disclaimer chapter" data-ui><Alert tone="warning" title="A fair warning.">Flip is a parody. It contains no neural networks, just a coin and TypeSafe's confidence formulas, implemented faithfully. Do not route production decisions through a coin. Or do; you were going to anyway.</Alert></div>
      </main>
      <Footer />
    </>
  );
}
