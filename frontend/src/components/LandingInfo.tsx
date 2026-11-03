import { FAQ } from "../lib/faq";
import { REPO_URL } from "../lib/repo";

const STEPS = [
  { title: "Enter your username", text: "Your Chess.com or Lichess name, or paste a PGN from anywhere." },
  { title: "Pick a game", text: "Your recent games are listed. Choose the one you want to look at." },
  { title: "Read the review", text: "Every move graded, the key moments marked, and a coach that explains them." },
];

/** What the review is, under the import form: plain text a reader (and a search engine) can find. */
export function LandingInfo() {
  return (
    <div className="landing-info">
      <section className="landing-info__steps" aria-labelledby="how-title">
        <h2 id="how-title" className="landing-info__heading">
          How it works
        </h2>
        <ol>
          {STEPS.map((step) => (
            <li key={step.title}>
              <h3>{step.title}</h3>
              <p>{step.text}</p>
            </li>
          ))}
        </ol>
      </section>

      <section className="landing-info__features" aria-label="What you get">
        <article>
          <h2 className="landing-info__heading">Every move graded</h2>
          <p>
            Each move is graded from Brilliant to Blunder, including Great, Best, Book, Inaccuracy, Mistake and Miss. You
            get accuracy for each player and for each phase of the game, an evaluation graph, and the moments that
            decided it.
          </p>
        </article>
        <article>
          <h2 className="landing-info__heading">A coach that explains why</h2>
          <p>
            The coach names what really happened: the fork, the pin, the piece left hanging, the threat you missed. It
            shows the line that proves it, and every sentence is checked against the board before you see it.
          </p>
        </article>
        <article>
          <h2 className="landing-info__heading">Try your own moves</h2>
          <p>
            Wondering what else you could have played? Make any move on the board. It is graded and explained the same
            way, with a live Stockfish engine running in your browser.
          </p>
        </article>
        <article>
          <h2 className="landing-info__heading">Free and open source</h2>
          <p>
            KnightReview is free, with no daily limit and no account. The code is open source under the GPL-3.0, so
            you can read it or{" "}
            <a href={REPO_URL} target="_blank" rel="noreferrer">
              run your own copy with Docker
            </a>
            .
          </p>
        </article>
      </section>

      <section className="landing-info__faq" aria-labelledby="faq-title">
        <h2 id="faq-title" className="landing-info__heading">
          Questions
        </h2>
        {FAQ.map((entry) => (
          <details key={entry.q}>
            <summary>{entry.q}</summary>
            <p>{entry.a}</p>
          </details>
        ))}
      </section>
    </div>
  );
}
