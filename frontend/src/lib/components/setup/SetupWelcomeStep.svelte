<script lang="ts">
  import { animeFly } from "$lib/animations";

  interface Props {
    onNext?: () => void;
  }

  let { onNext }: Props = $props();
</script>

<div class="panel-content" in:animeFly={{ y: 20, duration: 400 }}>
  <div class="features-grid">
    <div class="feature-item">
      <div class="feature-icon"><span class="material-icons">bolt</span></div>
      <h3>High-Speed Engine</h3>
      <p>Optimized download manager with resume support</p>
    </div>
    <div class="feature-item">
      <div class="feature-icon"><span class="material-icons">hub</span></div>
      <h3>Smart Integration</h3>
      <p>Seamless connection with Sonarr, Radarr & Jellyfin</p>
    </div>
    <div class="feature-item">
      <div class="feature-icon"><span class="material-icons">auto_awesome</span></div>
      <h3>Auto Detection</h3>
      <p>Intelligent media parsing and organization</p>
    </div>
  </div>

  <button class="btn-primary-large" onclick={onNext}>BEGIN SETUP <span class="material-icons">arrow_forward</span></button>
</div>

<style>
  .panel-content {
    padding: 3rem;
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }

  .features-grid {
    display: grid;
    grid-template-columns: repeat(3,1fr);
    gap: 1.5rem;
    margin-bottom: 2.5rem;
  }

  .feature-item {
    background: linear-gradient(145deg,#ffffff08,#ffffff03);
    border: 1px solid rgba(255,255,255,.08);
    border-radius: 16px;
    padding: 2rem 1.5rem;
    text-align: center;
    transition: all .3s ease;
    backdrop-filter: blur(10px);
  }

  .feature-item:hover {
    background: linear-gradient(145deg,#00f3ff14,#00f3ff08);
    border-color: #00f3ff4d;
    transform: translateY(-4px);
    box-shadow: 0 10px 30px #00f3ff1a;
  }

  .feature-icon {
    width: auto;
    height: auto;
    margin: 0 auto 1rem;
    display: inline-block;
    position: relative;
  }

  .feature-icon:before {
    display: none;
  }

  .feature-icon .material-icons {
    font-size: 2.5rem;
    color: var(--color-primary);
    padding: 10px;
    border-radius: 16px;
    display: block;
    filter: drop-shadow(0 0 1px rgba(0,243,255,.6)) drop-shadow(0 0 15px rgba(0,243,255,.4)) drop-shadow(0 0 30px rgba(0,243,255,.2));
  }

  .feature-item h3 {
    font-size: 1rem;
    font-weight: 700;
    margin: 0 0 .5rem;
    color: #fff;
  }

  .feature-item p {
    font-size: .85rem;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
  }

  .btn-primary-large {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: .5rem;
    padding: 1.125rem 2.5rem;
    font-weight: 900;
    font-size: .8rem;
    letter-spacing: .1em;
    border-radius: 10px;
    cursor: pointer;
    transition: all .2s;
    font-family: var(--font-mono);
    border: 2px solid var(--color-primary);
    background: linear-gradient(135deg,#00f3ff33,#00f3ff1a);
    color: var(--color-primary);
    box-shadow: 0 0 20px #00f3ff4d;
  }

  .btn-primary-large:hover {
    background: linear-gradient(135deg,#00ffa333,#00ffa31a);
    border-color: var(--color-secondary);
    color: var(--color-secondary);
    box-shadow: 0 0 30px #00ffa366;
    transform: translateY(-2px);
  }

  @media (max-width:768px) {
    .panel-content {
      padding: 2rem 1.5rem;
    }
    .features-grid {
      grid-template-columns: 1fr;
    }
    .btn-primary-large {
      width: 100%;
    }
  }

</style>
