module Fly

  module Console

    extend Component

    word '.', :dot do
      puts drop
    end

    word 'show' do
      puts top
    end

    word 'shell' do
      system drop
    end

  end

end
